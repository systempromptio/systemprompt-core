//! DB-backed tests for the `eval_*` repositories and the sampling reader.
//! Seeded `ai_requests` rows are namespaced per test with fresh UUIDs and
//! deleted afterwards, so assertions never depend on shared-table state.

use systemprompt_evaluation::{
    EvalCaseRepository, EvalResultRepository, EvalRubricRepository, EvalRunKind, EvalRunRepository,
    NewCaseParams, NewResultParams, NewRunParams, Rubric, RubricDimension, SampleFilter,
    SampleMode, SamplingRepository, TriggerSource, Verdict,
};
use systemprompt_identifiers::{AiRequestId, EvalRubricId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn new_run_params() -> NewRunParams {
    NewRunParams {
        kind: EvalRunKind::Judge,
        judge_provider: "anthropic".to_owned(),
        judge_model: "claude-sonnet-5".to_owned(),
        sample_size: 5,
        created_by: UserId::new("system"),
        rubric_id: None,
        trigger_source: TriggerSource::Manual,
    }
}

async fn seed_ai_request(pool: &systemprompt_database::DbPool, actor_kind: &str) -> AiRequestId {
    let id = format!("eval-test-{}", Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, status, actor_kind, actor_id)
         VALUES ($1, $1, 'system', '00000000-0000-0000-0000-00000000c0de', 'anthropic', 'claude-sonnet-5', 'completed', $2, 'system')",
    )
    .bind(&id)
    .bind(actor_kind)
    .execute(write.as_ref())
    .await
    .expect("seed request");
    for (seq, (role, content)) in [("user", "question"), ("assistant", "answer")]
        .into_iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&id)
        .bind(role)
        .bind(content)
        .bind(i32::try_from(seq).expect("seq"))
        .execute(write.as_ref())
        .await
        .expect("seed message");
    }
    AiRequestId::new(id)
}

async fn delete_ai_request(pool: &systemprompt_database::DbPool, id: &AiRequestId) {
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .execute(write.as_ref())
        .await
        .expect("delete request");
}

#[tokio::test]
async fn run_lifecycle_create_score_complete() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let runs = EvalRunRepository::new(&pool).expect("repo");

    let run_id = runs.create(&new_run_params()).await.expect("create");
    runs.record_scored(&run_id, true, 7).await.expect("scored");
    runs.record_scored(&run_id, false, 3).await.expect("scored");
    runs.complete(&run_id).await.expect("complete");

    let run = runs.get(&run_id).await.expect("get");
    assert_eq!(run.scored_count, 2);
    assert_eq!(run.failed_count, 1);
    assert_eq!(run.cost_microdollars, 10);
    assert_eq!(run.status.as_str(), "completed");
    assert!(run.completed_at.is_some());
}

#[tokio::test]
async fn failed_run_records_error_message() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let runs = EvalRunRepository::new(&pool).expect("repo");

    let run_id = runs.create(&new_run_params()).await.expect("create");
    runs.fail(&run_id, "budget exhausted").await.expect("fail");

    let run = runs.get(&run_id).await.expect("get");
    assert_eq!(run.status.as_str(), "failed");
    assert_eq!(run.error_message.as_deref(), Some("budget exhausted"));
}

#[tokio::test]
async fn rubric_upserts_and_reads_back() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let rubrics = EvalRubricRepository::new(&pool).expect("repo");

    let name = format!("rubric-{}", Uuid::new_v4());
    let rubric = Rubric {
        id: EvalRubricId::generate(),
        name: name.clone(),
        dimensions: vec![RubricDimension {
            name: "correctness".to_owned(),
            description: "accurate".to_owned(),
            weight: 1.0,
        }],
        pass_threshold: 4,
        prompt_template: None,
        enabled: true,
    };
    rubrics.upsert(&rubric).await.expect("upsert");

    let loaded = rubrics.get_by_name(&name).await.expect("get");
    assert_eq!(loaded.dimensions.len(), 1);
    assert_eq!(loaded.pass_threshold, 4);
}

#[tokio::test]
async fn results_track_failures_and_repair() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let runs = EvalRunRepository::new(&pool).expect("runs");
    let results = EvalResultRepository::new(&pool).expect("results");

    let run_id = runs.create(&new_run_params()).await.expect("create");
    let result_id = results
        .insert(&NewResultParams {
            run_id: run_id.clone(),
            ai_request_id: Some(AiRequestId::new(format!("eval-test-{}", Uuid::new_v4()))),
            case_id: None,
            provider: "anthropic".to_owned(),
            model: "claude-sonnet-5".to_owned(),
            overall_score: Some(2),
            dimension_scores: serde_json::json!([]),
            verdict: Verdict::Fail,
            rationale: Some("missed the point".to_owned()),
            repair_hint: Some("cite the source".to_owned()),
            prompt_excerpt: None,
            response_excerpt: None,
            judge_cost_microdollars: 3,
            repaired: false,
            replay_of_result_id: None,
            judge_ai_request_id: None,
        })
        .await
        .expect("insert");

    let failures = results
        .failures_for_replay(&run_id)
        .await
        .expect("failures");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].id, result_id);

    results.mark_repaired(&result_id).await.expect("repair");
    let failures = results
        .failures_for_replay(&run_id)
        .await
        .expect("failures");
    assert!(failures.is_empty());
}

#[tokio::test]
async fn sampling_excludes_job_actor_requests() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let sampling = SamplingRepository::new(&pool).expect("repo");

    let user_request = seed_ai_request(&pool, "user").await;
    let job_request = seed_ai_request(&pool, "job").await;

    let filter = SampleFilter::with_limit(10).ids(vec![
        user_request.as_str().to_owned(),
        job_request.as_str().to_owned(),
    ]);
    let sampled = sampling.sample(&filter).await.expect("sample");

    assert!(sampled.iter().any(|r| r.ai_request_id == user_request));
    assert!(
        sampled.iter().all(|r| r.ai_request_id != job_request),
        "job-actor request must never be sampled"
    );
    let user_row = sampled
        .iter()
        .find(|r| r.ai_request_id == user_request)
        .expect("user row");
    assert_eq!(user_row.response_text.as_deref(), Some("answer"));
    assert_eq!(user_row.messages.len(), 1);

    delete_ai_request(&pool, &user_request).await;
    delete_ai_request(&pool, &job_request).await;
}

#[tokio::test]
async fn cases_promote_and_toggle() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let cases = EvalCaseRepository::new(&pool).expect("repo");
    let sampling = SamplingRepository::new(&pool).expect("sampling");

    let request = seed_ai_request(&pool, "user").await;
    let filter = SampleFilter::with_limit(1).ids(vec![request.as_str().to_owned()]);
    let sampled = sampling.sample(&filter).await.expect("sample");
    let prompt = sampled.first().expect("sampled").canonical_prompt();

    let name = format!("case-{}", Uuid::new_v4());
    let case_id = cases
        .create(&NewCaseParams {
            name: name.clone(),
            prompt,
            source_ai_request_id: Some(request.clone()),
            expectation: Some("answers the question".to_owned()),
            tags: vec!["smoke".to_owned()],
            created_by: UserId::new("system"),
            prepared_body_sha256: None,
        })
        .await
        .expect("create");

    let listed = cases.list_enabled().await.expect("list");
    let case = listed.iter().find(|c| c.id == case_id).expect("case");
    assert_eq!(case.name, name);
    assert_eq!(case.provider.as_deref(), Some("anthropic"));

    cases
        .set_repair_hint(&case_id, "cite the source")
        .await
        .expect("hint");
    cases.set_enabled(&case_id, false).await.expect("disable");
    let listed = cases.list_enabled().await.expect("list");
    assert!(listed.iter().all(|c| c.id != case_id));

    delete_ai_request(&pool, &request).await;
}

struct ContextSeed<'a> {
    context_id: &'a str,
    minutes_ago: i64,
    synthetic: bool,
}

async fn seed_context_request(
    pool: &systemprompt_database::DbPool,
    seed: ContextSeed<'_>,
) -> AiRequestId {
    let id = format!("eval-test-{}", Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, status, actor_kind, actor_id, synthetic, created_at)
         VALUES ($1, $1, 'system', $2, 'anthropic', 'claude-sonnet-5', 'completed', 'user', 'system', $3, NOW() - ($4::int * INTERVAL '1 minute'))",
    )
    .bind(&id)
    .bind(seed.context_id)
    .bind(seed.synthetic)
    .bind(i32::try_from(seed.minutes_ago).expect("minutes"))
    .execute(write.as_ref())
    .await
    .expect("seed request");
    AiRequestId::new(id)
}

#[tokio::test]
async fn conversation_sampling_returns_latest_row_per_context() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let sampling = SamplingRepository::new(&pool).expect("repo");

    let ctx_a = Uuid::new_v4().to_string();
    let ctx_b = Uuid::new_v4().to_string();
    let mut seeded = Vec::new();
    let a_old = seed_context_request(
        &pool,
        ContextSeed {
            context_id: &ctx_a,
            minutes_ago: 30,
            synthetic: false,
        },
    )
    .await;
    let a_latest = seed_context_request(
        &pool,
        ContextSeed {
            context_id: &ctx_a,
            minutes_ago: 5,
            synthetic: false,
        },
    )
    .await;
    let a_synthetic = seed_context_request(
        &pool,
        ContextSeed {
            context_id: &ctx_a,
            minutes_ago: 1,
            synthetic: true,
        },
    )
    .await;
    let b_latest = seed_context_request(
        &pool,
        ContextSeed {
            context_id: &ctx_b,
            minutes_ago: 10,
            synthetic: false,
        },
    )
    .await;
    seeded.extend([
        a_old.clone(),
        a_latest.clone(),
        a_synthetic.clone(),
        b_latest.clone(),
    ]);

    let ids: Vec<String> = seeded.iter().map(|r| r.as_str().to_owned()).collect();
    let filter = SampleFilter::with_limit(10)
        .ids(ids)
        .mode(SampleMode::Conversation);
    let sampled = sampling.sample(&filter).await.expect("sample");

    assert_eq!(sampled.len(), 2, "one row per context: {sampled:?}");
    let sampled_ids: Vec<&str> = sampled.iter().map(|r| r.ai_request_id.as_str()).collect();
    assert!(sampled_ids.contains(&a_latest.as_str()), "{sampled_ids:?}");
    assert!(sampled_ids.contains(&b_latest.as_str()), "{sampled_ids:?}");
    assert!(
        !sampled_ids.contains(&a_synthetic.as_str()),
        "synthetic rows must be excluded: {sampled_ids:?}"
    );
    let a_row = sampled
        .iter()
        .find(|r| r.ai_request_id == a_latest)
        .expect("context A row");
    assert_eq!(a_row.context_id.as_str(), ctx_a);

    let scoped = sampling
        .sample(
            &SampleFilter::with_limit(10)
                .context_id(systemprompt_identifiers::ContextId::new_unchecked(
                    ctx_a.clone(),
                ))
                .mode(SampleMode::Conversation),
        )
        .await
        .expect("scoped sample");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].ai_request_id, a_latest);

    for id in &seeded {
        delete_ai_request(&pool, id).await;
    }
}
