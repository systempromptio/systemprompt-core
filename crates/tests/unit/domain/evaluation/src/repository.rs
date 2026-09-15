//! DB-backed tests for the golden-case repository and the sampling seam
//! (`AiRequestTrace`, served by the AI domain's request repository). Seeded
//! `ai_requests` rows are namespaced per test with fresh UUIDs and deleted
//! afterwards, so assertions never depend on shared-table state.

use systemprompt_evaluation::{CanonicalPrompt, EvalCaseRepository, NewCaseParams};
use systemprompt_identifiers::{AiRequestId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{TraceSampleFilter, TraceSampleMode};
use uuid::Uuid;

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
async fn sampling_excludes_job_actor_requests() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let sampling = crate::seams::trace(&pool);

    let user_request = seed_ai_request(&pool, "user").await;
    let job_request = seed_ai_request(&pool, "job").await;

    let filter =
        TraceSampleFilter::with_limit(10).ids(vec![user_request.clone(), job_request.clone()]);
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
    let sampling = crate::seams::trace(&pool);

    let request = seed_ai_request(&pool, "user").await;
    let filter = TraceSampleFilter::with_limit(1).ids(vec![request.clone()]);
    let sampled = sampling.sample(&filter).await.expect("sample");
    let prompt = CanonicalPrompt::from_sample(sampled.first().expect("sampled"));

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
    assert_eq!(case.prompt.provider.as_str(), "anthropic");
    assert_eq!(case.prompt.messages.len(), 1);

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
    let sampling = crate::seams::trace(&pool);

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

    let filter = TraceSampleFilter::with_limit(10)
        .ids(seeded.clone())
        .mode(TraceSampleMode::Conversation);
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
            &TraceSampleFilter::with_limit(10)
                .context_id(
                    systemprompt_identifiers::ContextId::try_new(ctx_a.clone())
                        .expect("valid ContextId"),
                )
                .mode(TraceSampleMode::Conversation),
        )
        .await
        .expect("scoped sample");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].ai_request_id, a_latest);

    for id in &seeded {
        delete_ai_request(&pool, id).await;
    }
}
