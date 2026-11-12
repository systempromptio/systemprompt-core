//! Retention holds against a real run. The logs window is overridden to one
//! day so the rows under test are a few days old: younger than every default
//! window, so no concurrent run without a hold can delete them, and older
//! than a fresh row another test seeds. The scenarios run in one test because
//! each enforced run prunes `logs` globally.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_extension::{RetentionHolds, RetentionTarget};
use systemprompt_scheduler::DatabaseCleanupJob;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Dependencies, Job, JobContext, JobResult};

use crate::retention_holds::{failing, hold};

const STALE_DAYS: i32 = 4000;

async fn run(pool: &systemprompt_database::DbPool, url: &str, holds: RetentionHolds) -> JobResult {
    use systemprompt_identifiers::{Actor, UserId};

    let app_ctx = test_app_context(pool, url);
    let dependencies = Dependencies::new()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app_ctx)
        .with(holds);
    let mut params = HashMap::new();
    params.insert("log_retention_days".to_owned(), "1".to_owned());
    let ctx = JobContext::new(
        Actor::job(UserId::new("retention-holds-admin"), "test".to_owned()),
        dependencies,
    )
    .with_enforce(true)
    .with_parameters(params);
    DatabaseCleanupJob
        .execute(&ctx)
        .await
        .expect("enforced retention run")
}

async fn seed_log(pool: &sqlx::PgPool, id: &str, days_ago: i32) {
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id) \
         VALUES ($1, NOW() - make_interval(days => $2), 'INFO', 'retention_holds', 'm', 'system')",
    )
    .bind(id)
    .bind(days_ago)
    .execute(pool)
    .await
    .expect("seed log row");
}

async fn log_exists(pool: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM logs WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("probe log row")
}

async fn analytics_exists(pool: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM analytics_events WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("probe analytics row")
}

#[tokio::test]
async fn holds_clamp_withhold_and_compose_on_an_enforced_run() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let id = |name: &str| format!("hold_{name}_{suffix}");

    seed_log(&pg, &id("unheld"), 2).await;
    let result = run(&pool, &url, RetentionHolds::default()).await;
    assert!(result.success, "{:?}", result.message);
    assert!(
        !log_exists(&pg, &id("unheld")).await,
        "without a hold a row past the window is deleted"
    );

    seed_log(&pg, &id("single_before"), 4).await;
    seed_log(&pg, &id("single_after"), 2).await;
    let watermark = Utc::now() - Duration::days(3);
    let result = run(
        &pool,
        &url,
        RetentionHolds::new(vec![hold("exporter", RetentionTarget::Logs, watermark)]),
    )
    .await;
    assert!(result.success, "{:?}", result.message);
    assert!(!log_exists(&pg, &id("single_before")).await);
    assert!(
        log_exists(&pg, &id("single_after")).await,
        "a row at or after the watermark survives"
    );
    let message = result.message.unwrap_or_default();
    assert!(message.contains("@held:exporter"), "{message}");

    seed_log(&pg, &id("pair_before"), 6).await;
    seed_log(&pg, &id("pair_between"), 4).await;
    let result = run(
        &pool,
        &url,
        RetentionHolds::new(vec![
            hold(
                "late",
                RetentionTarget::Logs,
                Utc::now() - Duration::days(3),
            ),
            hold(
                "early",
                RetentionTarget::Logs,
                Utc::now() - Duration::days(5),
            ),
        ]),
    )
    .await;
    assert!(result.success, "{:?}", result.message);
    assert!(!log_exists(&pg, &id("pair_before")).await);
    assert!(
        log_exists(&pg, &id("pair_between")).await,
        "the earliest watermark wins"
    );

    seed_log(&pg, &id("withheld"), 2).await;
    let stale_event = id("stale_event");
    sqlx::query(
        "INSERT INTO analytics_events (id, user_id, event_type, event_category, severity, timestamp) \
         VALUES ($1, 'system', 'retention_hold_test', 'test', 'info', NOW() - make_interval(days => $2))",
    )
    .bind(&stale_event)
    .bind(STALE_DAYS)
    .execute(&*pg)
    .await
    .expect("seed analytics event");
    let result = run(
        &pool,
        &url,
        RetentionHolds::new(vec![failing("broken", RetentionTarget::Logs)]),
    )
    .await;
    assert!(result.success, "{:?}", result.message);
    assert_eq!(result.items_failed, Some(1), "one target withheld");
    let message = result.message.unwrap_or_default();
    assert!(message.contains("logs=withheld:broken"), "{message}");
    assert!(
        log_exists(&pg, &id("withheld")).await,
        "a failing hold withholds its target"
    );
    assert!(
        !analytics_exists(&pg, &stale_event).await,
        "other targets are still pruned"
    );

    sqlx::query("DELETE FROM logs WHERE id LIKE $1")
        .bind(format!("hold_%_{suffix}"))
        .execute(&*pg)
        .await
        .ok();
}
