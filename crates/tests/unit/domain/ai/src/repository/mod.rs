// DB-backed tests for the AI domain repository layer.

mod ai_gateway_policies;
mod ai_quota_buckets;
mod ai_request_payloads;
mod ai_requests;
mod ai_safety_findings;

use systemprompt_ai::models::AiRequestRecord;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};

// Acquire a migrated test pool, or `None` when DATABASE_URL is unset so the
// shard skips DB-backed tests cleanly.
pub(crate) async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(pool)
}

// Seed a user and a pending ai_requests row so child tables (payloads,
// safety findings, messages, tool calls) have a valid FK target.
pub(crate) async fn seed_request(pool: &DbPool, user_id: &UserId) -> AiRequestId {
    let email = format!("{}@ai.invalid", user_id.as_str());
    seed_user_row(pool, user_id, &email)
        .await
        .expect("seed user");
    let repo = systemprompt_ai::repository::AiRequestRepository::new(pool).expect("repo");
    let record = AiRequestRecord::builder(AiRequestId::generate(), user_id.clone())
        .provider("anthropic")
        .model("claude-3-opus")
        .build()
        .expect("record");
    repo.insert(&record).await.expect("insert request")
}

pub(crate) fn user() -> UserId {
    unique_user_id("ai-repo")
}

pub(crate) fn completed_record(user_id: &UserId) -> AiRequestRecord {
    AiRequestRecord::builder(AiRequestId::generate(), user_id.clone())
        .provider("anthropic")
        .model("claude-3-opus")
        .tokens(Some(100), Some(50))
        .cache(true, Some(20), Some(10))
        .streaming(false)
        .cost(1_500)
        .latency(420)
        .completed()
        .build()
        .expect("record")
}
