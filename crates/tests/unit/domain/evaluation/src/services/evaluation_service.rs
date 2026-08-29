//! `EvaluationService` and, through it, the `AutoImproveLoop`.
//!
//! The loop's fields are `pub(super)`, so it cannot be constructed from here.
//! Driving it through `run_judge` is not a workaround: it is the only way the
//! loop is ever entered in production, and one call covers the service, the
//! loop, the judge and the replay path together.
//!
//! The provider is a queue, so the number of queued answers is what selects
//! the path under test. A passing verdict ends the round in one call; a
//! failing verdict with a repair hint costs three — judge, replay, re-judge.

use systemprompt_evaluation::models::{SampleFilter, TriggerSource};
use systemprompt_evaluation::services::RunRequest;
use systemprompt_identifiers::{EvalRunId, UserId};

use super::support::{delete_ai_request, seed_ai_request, service_answering, verdict_json};

fn run_request(budget: Option<i64>) -> RunRequest {
    RunRequest {
        judge_provider: "mock".to_owned(),
        judge_model: "mock-judge".to_owned(),
        rubric_name: None,
        filter: SampleFilter::with_limit(10),
        budget_microdollars: budget,
        created_by: UserId::new("eval-svc-test"),
        trigger_source: TriggerSource::Manual,
    }
}

// Why: `rubric_name: None` resolves the default rubric, seeding it on a clean
// database. A run that cannot resolve a rubric has nothing to grade against.
#[tokio::test]
async fn a_run_with_no_rubric_named_resolves_the_default() {
    let (service, _pool) = service_answering(&[]).await;

    let (run_id, report) = service
        .run_judge(run_request(None))
        .await
        .expect("a run with no rubric named should seed and use the default");

    assert_eq!(report.scored, 0, "nothing was sampled, so nothing scored");

    let run = service
        .get_run(&run_id)
        .await
        .expect("the run should exist");
    assert!(
        run.rubric_id.is_some(),
        "the run should record which rubric it graded against"
    );
}

#[tokio::test]
async fn a_passing_verdict_is_scored_once_and_not_replayed() {
    let (service, pool) = service_answering(&[verdict_json(5, None)]).await;
    let seeded = seed_ai_request(&pool).await;

    let mut request = run_request(None);
    request.filter = request.filter.ids(vec![seeded.as_str().to_owned()]);

    let (run_id, report) = service.run_judge(request).await.expect("run_judge");

    assert_eq!(report.scored, 1, "the seeded request should be judged");
    assert_eq!(report.failed, 0, "a score of 5 is not a failure");
    assert_eq!(
        report.replayed, 0,
        "a passing verdict must not trigger a replay"
    );

    let results = service.list_results(&run_id).await.expect("list_results");
    assert_eq!(results.len(), 1, "one judged request, one result row");

    delete_ai_request(&pool, &seeded).await;
}

// Why: this is the whole point of the loop. A failing verdict carrying a
// repair hint is replayed with that hint and re-judged, and only a passing
// re-judge counts as repaired. Without the hint there is nothing to apply, so
// the round stops after the first verdict.
#[tokio::test]
async fn a_failure_with_a_repair_hint_is_replayed_and_counted_as_repaired() {
    let (service, pool) = service_answering(&[
        verdict_json(1, Some("cite the source")),
        "a better answer".to_owned(),
        verdict_json(5, None),
    ])
    .await;
    let seeded = seed_ai_request(&pool).await;

    let mut request = run_request(None);
    request.filter = request.filter.ids(vec![seeded.as_str().to_owned()]);

    let (run_id, report) = service.run_judge(request).await.expect("run_judge");

    assert_eq!(report.failed, 1, "a score of 1 is a failure");
    assert_eq!(report.replayed, 1, "the failure should have been replayed");
    assert_eq!(
        report.repaired, 1,
        "a passing re-judge should count as repaired"
    );

    let results = service.list_results(&run_id).await.expect("list_results");
    assert_eq!(
        results.len(),
        2,
        "the original verdict and the replayed one are both recorded"
    );

    delete_ai_request(&pool, &seeded).await;
}

// Why: a replay that still fails is replayed but NOT repaired. Counting every
// replay as a repair would report the loop fixing things it did not fix, and a
// test whose re-judge always passes cannot tell the two apart.
#[tokio::test]
async fn a_replay_that_still_fails_is_not_counted_as_repaired() {
    let (service, pool) = service_answering(&[
        verdict_json(1, Some("cite the source")),
        "still a poor answer".to_owned(),
        verdict_json(1, None),
    ])
    .await;
    let seeded = seed_ai_request(&pool).await;

    let mut request = run_request(None);
    request.filter = request.filter.ids(vec![seeded.as_str().to_owned()]);

    let (_run_id, report) = service.run_judge(request).await.expect("run_judge");

    assert_eq!(report.replayed, 1, "the failure was replayed");
    assert_eq!(
        report.repaired, 0,
        "a replay that still fails must not be counted as repaired"
    );

    delete_ai_request(&pool, &seeded).await;
}

#[tokio::test]
async fn a_failure_without_a_repair_hint_is_not_replayed() {
    let (service, pool) = service_answering(&[verdict_json(1, None)]).await;
    let seeded = seed_ai_request(&pool).await;

    let mut request = run_request(None);
    request.filter = request.filter.ids(vec![seeded.as_str().to_owned()]);

    let (_run_id, report) = service.run_judge(request).await.expect("run_judge");

    assert_eq!(report.failed, 1);
    assert_eq!(
        report.replayed, 0,
        "with no hint there is no correction to apply, so no replay"
    );

    delete_ai_request(&pool, &seeded).await;
}

// Why: a judge failure must not abort the whole run. One unparseable response
// among many would otherwise discard every verdict already scored.
#[tokio::test]
async fn a_judge_failure_is_swallowed_so_the_run_completes() {
    let (service, pool) = service_answering(&["not json at all".to_owned()]).await;
    let seeded = seed_ai_request(&pool).await;

    let mut request = run_request(None);
    request.filter = request.filter.ids(vec![seeded.as_str().to_owned()]);

    let (_run_id, report) = service
        .run_judge(request)
        .await
        .expect("an unparseable verdict must not fail the whole run");

    assert_eq!(report.scored, 0, "the unparseable verdict scored nothing");

    delete_ai_request(&pool, &seeded).await;
}

#[tokio::test]
async fn replaying_a_run_with_no_failures_is_refused() {
    let (service, _pool) = service_answering(&[]).await;

    let (run_id, _) = service
        .run_judge(run_request(None))
        .await
        .expect("run_judge");

    let err = service
        .replay_failures(&run_id, run_request(None))
        .await
        .expect_err("a run with no failures has nothing to replay");

    assert!(
        format!("{err}").contains(run_id.as_str()),
        "the error should name the run it could not replay: {err}"
    );
}

#[tokio::test]
async fn a_run_that_does_not_exist_is_reported_rather_than_returning_empty() {
    let (service, _pool) = service_answering(&[]).await;

    let missing = EvalRunId::new("eval-run-that-does-not-exist");
    service
        .get_run(&missing)
        .await
        .expect_err("an unknown run id must not resolve to a run");
}

#[tokio::test]
async fn recent_runs_include_the_one_just_created() {
    let (service, _pool) = service_answering(&[]).await;

    let (run_id, _) = service
        .run_judge(run_request(None))
        .await
        .expect("run_judge");

    let runs = service.list_runs(50).await.expect("list_runs");
    assert!(
        runs.iter().any(|r| r.id == run_id),
        "the run just created should appear in the recent list"
    );
}
