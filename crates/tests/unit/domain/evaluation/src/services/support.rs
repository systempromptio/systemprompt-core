//! Shared fixtures for the evaluation service tests.
//!
//! Deliberately fails loudly rather than returning `None` when the database is
//! unavailable. The sibling `repository.rs` suite uses
//! `let Ok(url) = fixture_database_url() else { return }`, which reports green
//! having executed nothing — a misconfigured database silently passes it.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_evaluation::repository::EvalRepositories;
use systemprompt_evaluation::services::EvaluationService;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::ai::{AiResponse, DynAiProvider};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use systemprompt_test_mocks::MockAiProvider;
use uuid::Uuid;

pub async fn pool() -> DbPool {
    let b = ensure_test_bootstrap();
    fixture_db_pool(&b.database_url)
        .await
        .expect("the evaluation service tests need a reachable test database")
}

pub fn ai_response(content: &str) -> AiResponse {
    let mut resp = AiResponse::default();
    resp.request_id = Uuid::new_v4();
    resp.content = content.to_owned();
    resp.provider = "mock".to_owned();
    resp.model = "mock-judge".to_owned();
    resp
}

pub fn verdict_json(score: i32, repair_hint: Option<&str>) -> String {
    let hint = repair_hint.map_or("null".to_owned(), |h| format!("\"{h}\""));
    format!(
        r#"{{"overall_score":{score},"dimension_scores":[],"rationale":"because","repair_hint":{hint}}}"#
    )
}

/// Build a service whose provider answers with `contents` in order. The loop
/// judges, then replays, then re-judges, so a repair round needs three.
pub async fn service_answering(contents: &[String]) -> (EvaluationService, DbPool) {
    let pool = pool().await;
    let repos = EvalRepositories::new(&pool).expect("eval repositories");
    let mut builder = MockAiProvider::builder();
    for content in contents {
        builder = builder.with_generate_response(Ok(ai_response(content)));
    }
    let ai: DynAiProvider = Arc::new(builder.build());
    (EvaluationService::new(repos, ai), pool)
}

/// Seed a completed `ai_requests` row with a two-turn transcript, so the
/// sampler has something to select and the judge something to grade.
pub async fn seed_ai_request(pool: &DbPool) -> AiRequestId {
    let id = format!("eval-svc-{}", Uuid::new_v4());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, status, \
         actor_kind, actor_id, cost_microdollars)
         VALUES ($1, $1, 'system', '00000000-0000-0000-0000-00000000c0de', 'anthropic', \
         'claude-sonnet-5', 'completed', 'user', 'system', 0)",
    )
    .bind(&id)
    .execute(write.as_ref())
    .await
    .expect("seed ai_request");

    for (seq, (role, content)) in [("user", "the question"), ("assistant", "the answer")]
        .into_iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO ai_request_messages (request_id, sequence_number, role, content)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&id)
        .bind(i32::try_from(seq).expect("sequence fits i32"))
        .bind(role)
        .bind(content)
        .execute(write.as_ref())
        .await
        .expect("seed message");
    }

    AiRequestId::new(id)
}

pub async fn delete_ai_request(pool: &DbPool, id: &AiRequestId) {
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .execute(write.as_ref())
        .await
        .expect("delete ai_request");
}
