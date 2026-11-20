//! Retention holds against a real run. The logs window is overridden to one
//! day so the rows under test are a few days old: younger than every default
//! window, so no concurrent run without a hold can delete them, and older
//! than a fresh row another test seeds. The scenarios run in one test because
//! each enforced run prunes `logs` globally.

use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_extension::{RetentionDecl, RetentionHold, RetentionTarget};
use systemprompt_scheduler::JobRunReport;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

use crate::test_extension::{app_over, retention_job, run_admitted_job};

use crate::retention_holds::{failing, hold};

const STALE_DAYS: i32 = 4000;

async fn run(url: &str, holds: Vec<Arc<dyn RetentionHold>>) -> JobRunReport {
    let mut members = systemprompt_test_fixtures::fixture_core_members();
    members.push(Box::new(systemprompt_test_fixtures::Probe::new(
        "retention-holds-probe",
        move |c| {
            for hold in &holds {
                c.retention(RetentionDecl::Hold(Arc::clone(hold)));
            }
        },
    )));
    let (app, schedule) = app_over(
        url,
        systemprompt_test_fixtures::fixture_admitted_of(members),
    )
    .await;
    run_admitted_job(
        app,
        schedule,
        retention_job(true, &[("log_retention_days", "1")]),
    )
    .await
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
    let result = run(&url, Vec::new()).await;
    assert!(result.success, "{:?}", result.message);
    assert!(
        !log_exists(&pg, &id("unheld")).await,
        "without a hold a row past the window is deleted"
    );

    seed_log(&pg, &id("single_before"), 4).await;
    seed_log(&pg, &id("single_after"), 2).await;
    let watermark = Utc::now() - Duration::days(3);
    let result = run(
        &url,
        vec![hold("exporter", RetentionTarget::Logs, watermark)],
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
        &url,
        vec![
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
        ],
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
    let result = run(&url, vec![failing("broken", RetentionTarget::Logs)]).await;
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
