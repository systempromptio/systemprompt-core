//! `admin evals show` — the report a seeded run renders.
//!
//! `show::execute` returns the `CommandOutput` that the command tree renders
//! and discards, so these assert on the artifact itself. Driving the tree's
//! public `execute` instead would only prove it returned `Ok(())`, which it
//! does whether or not the report carries the right rows.
//!
//! The fixture, bootstrap and command context are shared with
//! `admin_evals_db`: `init_services_bootstrap` is process-global, so a second
//! `OnceLock` boot in this file would race the first.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_database::DbPool;

use super::admin_evals_db::{ctx, pool};

async fn seed_run(pool: &DbPool, scored: i32, failed: i32) -> String {
    let id = format!("eval-run-{}", uuid::Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO eval_runs (id, kind, status, judge_provider, judge_model, \
         sample_size, scored_count, failed_count, created_by) \
         VALUES ($1, 'judge', 'completed', 'anthropic', 'claude-fixture-1', $2, $2, $3, 'tests')",
    )
    .bind(&id)
    .bind(scored)
    .bind(failed)
    .execute(&*write)
    .await
    .expect("seed eval run");
    id
}

struct SeedResult<'a> {
    run_id: &'a str,
    verdict: &'a str,
    score: Option<i32>,
    rationale: Option<&'a str>,
    repair_hint: Option<&'a str>,
    replay_of: Option<&'a str>,
    repaired: bool,
}

async fn seed_result(pool: &DbPool, r: &SeedResult<'_>) -> String {
    let id = format!("eval-res-{}", uuid::Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO eval_results (id, run_id, provider, model, overall_score, verdict, \
         rationale, repair_hint, replay_of_result_id, repaired) \
         VALUES ($1, $2, 'anthropic', 'claude-fixture-1', $3, $4, $5, $6, $7, $8)",
    )
    .bind(&id)
    .bind(r.run_id)
    .bind(r.score)
    .bind(r.verdict)
    .bind(r.rationale)
    .bind(r.repair_hint)
    .bind(r.replay_of)
    .bind(r.repaired)
    .execute(&*write)
    .await
    .expect("seed eval result");
    id
}

async fn show(run_id: &str) -> serde_json::Value {
    let pool = pool().await;
    let output = systemprompt_cli::admin::evals::show::execute(
        systemprompt_cli::admin::evals::show::ShowArgs {
            run_id: run_id.to_owned(),
        },
        &ctx(&pool),
    )
    .await
    .expect("showing a seeded run should succeed");
    serde_json::json!({
        "title": output.title(),
        "artifact": serde_json::to_value(output.artifact()).expect("serialise artifact"),
    })
}

// Why: the title is the only place the run's own counters are reported. A
// reader uses them to decide whether a run is worth inspecting, so they have to
// be the run's real numbers rather than the length of the result list.
#[tokio::test]
async fn the_title_reports_the_runs_own_scored_and_failed_counts() {
    let pool = pool().await;
    let run_id = seed_run(&pool, 7, 2).await;

    let shown = show(&run_id).await;
    let title = shown["title"].as_str().expect("a titled table");

    assert!(
        title.contains("scored 7") && title.contains("failed 2"),
        "the title should carry the run's counters: {title}"
    );
    assert!(
        title.contains(&run_id),
        "the title should name the run being shown: {title}"
    );
}

// Why: every nullable column is rendered through `unwrap_or_default`. A NULL
// has to become an empty cell — the alternative is the string "null" appearing
// in a report an operator reads as a score.
#[tokio::test]
async fn a_result_with_no_score_or_rationale_renders_empty_cells_not_null() {
    let pool = pool().await;
    let run_id = seed_run(&pool, 0, 1).await;
    seed_result(
        &pool,
        &SeedResult {
            run_id: &run_id,
            verdict: "skipped",
            score: None,
            rationale: None,
            repair_hint: None,
            replay_of: None,
            repaired: false,
        },
    )
    .await;

    let shown = show(&run_id).await;
    let rows = shown["artifact"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items in artifact: {}", shown["artifact"]));
    assert_eq!(rows.len(), 1, "one seeded result, one row");

    for column in ["score", "rationale", "repair_hint", "replay_of"] {
        assert_eq!(
            rows[0][column].as_str(),
            Some(""),
            "a NULL {column} must render as an empty cell, not as {}",
            rows[0][column]
        );
    }
}

// Why: the graded fields are the whole point of the report. This pins that the
// values shown are the values stored, rather than defaults that happen to look
// plausible.
#[tokio::test]
async fn a_graded_result_renders_the_values_that_were_stored() {
    let pool = pool().await;
    let run_id = seed_run(&pool, 1, 0).await;
    seed_result(
        &pool,
        &SeedResult {
            run_id: &run_id,
            verdict: "pass",
            score: Some(4),
            rationale: Some("clear and correct"),
            repair_hint: Some("tighten the summary"),
            replay_of: None,
            repaired: true,
        },
    )
    .await;

    let shown = show(&run_id).await;
    let row = &shown["artifact"]["items"].as_array().expect("items")[0];

    assert_eq!(row["score"].as_str(), Some("4"));
    assert_eq!(row["verdict"].as_str(), Some("pass"));
    assert_eq!(row["rationale"].as_str(), Some("clear and correct"));
    assert_eq!(row["repair_hint"].as_str(), Some("tighten the summary"));
    assert_eq!(
        row["repaired"].as_bool(),
        Some(true),
        "the repaired flag distinguishes a repair from an original judgement"
    );
}

// Why: `show` scopes results to the run it was asked about. A query missing its
// run filter would render another run's results under this run's title, which
// reads as a correct report.
#[tokio::test]
async fn results_from_another_run_are_not_shown() {
    let pool = pool().await;
    let mine = seed_run(&pool, 1, 0).await;
    let theirs = seed_run(&pool, 1, 0).await;

    let mine_result = seed_result(
        &pool,
        &SeedResult {
            run_id: &mine,
            verdict: "pass",
            score: Some(5),
            rationale: None,
            repair_hint: None,
            replay_of: None,
            repaired: false,
        },
    )
    .await;
    seed_result(
        &pool,
        &SeedResult {
            run_id: &theirs,
            verdict: "fail",
            score: Some(1),
            rationale: None,
            repair_hint: None,
            replay_of: None,
            repaired: false,
        },
    )
    .await;

    let shown = show(&mine).await;
    let rows = shown["artifact"]["items"].as_array().expect("items");

    assert_eq!(
        rows.len(),
        1,
        "only this run's results belong in the report"
    );
    assert_eq!(rows[0]["id"].as_str(), Some(mine_result.as_str()));
}
