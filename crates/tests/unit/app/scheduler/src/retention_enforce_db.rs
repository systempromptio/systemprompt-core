//! The retention job under `enforce`: every table's batched delete runs, and
//! a row past its window is gone afterwards while a fresh one stays.


use systemprompt_scheduler::JobRunReport;
use systemprompt_test_fixtures::{test_database_url, test_db_pool};

use crate::test_extension::{retention_job, run_admitted_job, scheduled_app};

async fn run(url: &str, enforce: bool, parameters: &[(&str, &str)]) -> JobRunReport {
    let (app, schedule) = scheduled_app(url).await;
    run_admitted_job(app, schedule, retention_job(enforce, parameters)).await
}

async fn seed_log(pool: &sqlx::PgPool, id: &str, days_ago: i32) {
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id) \
         VALUES ($1, NOW() - make_interval(days => $2), 'INFO', 'retention_test', 'm', 'system')",
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

#[tokio::test]
async fn enforce_deletes_rows_past_their_window_and_keeps_fresh_ones() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let old = format!("ret_old_{suffix}");
    let fresh = format!("ret_fresh_{suffix}");
    seed_log(&pg, &old, 4000).await;
    seed_log(&pg, &fresh, 0).await;

    let result = run(&url, true, &[]).await;

    assert!(result.success, "{:?}", result.message);
    let message = result.message.unwrap_or_default();
    for table in [
        "logs=",
        "analytics_events=",
        "ai_request_messages=",
        "mcp_tool_executions=",
        "event_outbox=",
        "ai_request_payloads=",
        "governance_decisions=",
        "ai_quota_buckets=",
        "archived_users=",
    ] {
        assert!(message.contains(table), "{table} missing from {message}");
    }
    assert!(
        !log_exists(&pg, &old).await,
        "a 4000-day-old log must be deleted"
    );
    assert!(log_exists(&pg, &fresh).await, "a fresh log must survive");

    sqlx::query("DELETE FROM logs WHERE id = $1")
        .bind(&fresh)
        .execute(&*pg)
        .await
        .ok();
}

#[tokio::test]
async fn without_enforce_an_expired_row_is_kept() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let id = format!("ret_dry_{}", uuid::Uuid::new_v4().simple());
    seed_log(&pg, &id, 4000).await;

    let result = run(&url, false, &[("log_retention_days", "1")]).await;

    assert!(result.success, "{:?}", result.message);
    assert!(log_exists(&pg, &id).await, "a dry run deletes nothing");
    sqlx::query("DELETE FROM logs WHERE id = $1")
        .bind(&id)
        .execute(&*pg)
        .await
        .ok();
}
