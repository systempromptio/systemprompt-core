//! DB-backed `execute` paths for the two multi-replica reaper jobs added in
//! 0.44: `ServiceRegistryGcJob`, which evicts registry rows whose replica
//! stopped heartbeating, and `ThoughtSignatureCleanupJob`, which drops expired
//! gateway thought signatures.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, GatewayConversationId, UserId};
use systemprompt_scheduler::jobs::{ServiceRegistryGcJob, ThoughtSignatureCleanupJob};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};
use uuid::Uuid;

// Why: the registry reaper deletes by age across the whole table, so a cutoff
// anywhere near "now" would evict the live rows of every other test sharing the
// database. The test ages only its own row past this horizon and passes a
// retention just inside it, confining the sweep to what it seeded. Only one
// test may drive the reaper, or two concurrent sweeps each collect the other's
// aged row and neither can assert the count it caused.
const AGED_DAYS: i64 = 400;
const RETAIN_DAYS: i64 = 399;

async fn pool() -> DbPool {
    let url = fixture_database_url().expect("DATABASE_URL must be set for the reaper job tests");
    ensure_test_bootstrap();
    fixture_db_pool(&url).await.expect("fixture pool")
}

fn ctx(db_pool_any: Arc<dyn std::any::Any + Send + Sync>) -> JobContext {
    let actor = Actor::job(UserId::new("multi-replica-jobs-test"), "test".to_owned());
    JobContext::new(actor, db_pool_any, Arc::new(()), Arc::new(()))
}

fn retain_secs_param() -> HashMap<String, String> {
    HashMap::from([(
        "dead_after_secs".to_owned(),
        (RETAIN_DAYS * 86_400).to_string(),
    )])
}

async fn seed_service(pool: &DbPool, instance: &str, heartbeat_days_ago: i64) {
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, status, port, heartbeat_at) \
         VALUES ($1, 'gc-probe', 'gc-probe', 'running', 65535, \
         NOW() - make_interval(days => $2::int))",
    )
    .bind(instance)
    .bind(i32::try_from(heartbeat_days_ago).expect("days fits i32"))
    .execute(write.as_ref())
    .await
    .expect("seed service row");
}

async fn service_exists(pool: &DbPool, instance: &str) -> bool {
    let write = pool.write_pool_arc().expect("write pool");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM services WHERE instance_id = $1")
        .bind(instance)
        .fetch_one(write.as_ref())
        .await
        .expect("count service rows");
    count > 0
}

async fn drop_service(pool: &DbPool, instance: &str) {
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM services WHERE instance_id = $1")
        .bind(instance)
        .execute(write.as_ref())
        .await
        .expect("cleanup service row");
}

#[tokio::test]
async fn service_registry_gc_fails_without_a_db_pool_in_context() {
    ensure_test_bootstrap();
    let err = ServiceRegistryGcJob
        .execute(&ctx(Arc::new(())))
        .await
        .expect_err("a job with no pool must not report success");
    assert!(err.to_string().contains("DbPool"), "{err}");
}

#[tokio::test]
async fn service_registry_gc_reaps_a_silent_replica_and_spares_a_heartbeating_one() {
    let pool = pool().await;
    let dead = format!("gc-dead-{}", Uuid::new_v4().simple());
    let alive = format!("gc-alive-{}", Uuid::new_v4().simple());
    seed_service(&pool, &dead, AGED_DAYS).await;
    seed_service(&pool, &alive, 0).await;

    let result = ServiceRegistryGcJob
        .execute(&ctx(Arc::new(pool.clone())).with_parameters(retain_secs_param()))
        .await
        .expect("gc execute");

    assert!(result.success);
    assert!(
        !service_exists(&pool, &dead).await,
        "a replica silent for {AGED_DAYS} days must be evicted from the registry"
    );
    assert!(
        service_exists(&pool, &alive).await,
        "a heartbeating replica must survive the sweep"
    );
    assert!(
        result.items_failed.expect("gc reports a reaped count") >= 1,
        "the reaped row must be counted in the job's reaped stat, not silently dropped"
    );

    drop_service(&pool, &alive).await;
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
    let repo = AiThoughtSignatureRepository::new(&pool).expect("repo");
    let conv = GatewayConversationId::new_unchecked(&format!(
        "ctx_{:016x}",
        u64::from(Uuid::new_v4().as_u128() as u32)
    ));

    repo.upsert(&conv, "expired", "sig-expired", Duration::from_secs(3600))
        .await
        .expect("seed expired");
    repo.upsert(&conv, "live", "sig-live", Duration::from_secs(3600))
        .await
        .expect("seed live");

    let write = pool.write_pool_arc().expect("write pool");
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
        repo.find(&conv, "live", Duration::from_secs(3600))
            .await
            .expect("find live")
            .as_deref(),
        Some("sig-live"),
        "an unexpired signature must survive the sweep"
    );

    sqlx::query("DELETE FROM ai_gateway_thought_signatures WHERE conversation_id = $1")
        .bind(conv.as_str())
        .execute(write.as_ref())
        .await
        .expect("cleanup signatures");
}
