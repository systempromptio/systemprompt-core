//! DB-backed `execute` paths for `ThoughtSignatureCleanupJob`, which drops
//! expired gateway thought signatures.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_ai::repository::thought_signatures::ThoughtSignatureWrite;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, GatewayConversationId, UserId};
use systemprompt_scheduler::jobs::ThoughtSignatureCleanupJob;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use uuid::Uuid;

async fn pool() -> DbPool {
    let url = fixture_database_url().expect("DATABASE_URL must be set for the reaper job tests");
    ensure_test_bootstrap();
    fixture_db_pool(&url).await.expect("fixture pool")
}

fn ctx(db_pool_any: Arc<dyn std::any::Any + Send + Sync>) -> JobContext {
    let actor = Actor::job(UserId::new("multi-replica-jobs-test"), "test".to_owned());
    JobContext::new(actor, db_pool_any, Arc::new(()), Arc::new(()))
}

#[tokio::test]
async fn thought_signature_cleanup_fails_without_a_db_pool_in_context() {
    ensure_test_bootstrap();
    let err = ThoughtSignatureCleanupJob
        .execute(&ctx(Arc::new(())))
        .await
        .expect_err("a job with no pool must not report success");
    assert!(err.to_string().contains("DbPool"), "{err}");
}

#[tokio::test]
async fn thought_signature_cleanup_drops_expired_rows_and_keeps_live_ones() {
    let pool = pool().await;
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(write.as_ref())
        .await
        .expect("seed owner");
    let repo = AiThoughtSignatureRepository::new(&pool).expect("repo");
    let conv = GatewayConversationId::try_new(&format!(
        "ctx_{:016x}",
        u64::from(Uuid::new_v4().as_u128() as u32)
    ))
    .expect("valid GatewayConversationId");

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "expired",
        signature: "sig-expired",
        ttl: Duration::from_secs(3600),
    })
    .await
    .expect("seed expired");
    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "live",
        signature: "sig-live",
        ttl: Duration::from_secs(3600),
    })
    .await
    .expect("seed live");

    sqlx::query(
        "UPDATE ai_gateway_thought_signatures SET expires_at = NOW() - INTERVAL '1 hour' \
         WHERE conversation_id = $1 AND tool_use_id = 'expired'",
    )
    .bind(conv.as_str())
    .execute(write.as_ref())
    .await
    .expect("age the expired row");

    let result = ThoughtSignatureCleanupJob
        .execute(&ctx(Arc::new(pool.clone())))
        .await
        .expect("cleanup execute");

    assert!(result.success);
    // Why: `find` already filters on `expires_at > NOW()`, so an elapsed row
    // reads as absent whether or not it was deleted. Only a direct row count
    // distinguishes a sweep that ran from one that did nothing.
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_gateway_thought_signatures \
         WHERE conversation_id = $1 AND tool_use_id = 'expired'",
    )
    .bind(conv.as_str())
    .fetch_one(write.as_ref())
    .await
    .expect("count expired rows");
    assert_eq!(
        remaining, 0,
        "an elapsed signature must be deleted from the table, not merely hidden by the \
         read filter"
    );
    assert_eq!(
        repo.find(&user_id, &conv, "live", Duration::from_secs(3600))
            .await
            .expect("find live")
            .as_deref(),
        Some("sig-live"),
        "an unexpired signature must survive the sweep"
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id.as_str())
        .execute(write.as_ref())
        .await
        .expect("cleanup signatures");
}
