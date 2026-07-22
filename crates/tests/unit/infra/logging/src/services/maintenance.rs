//! DB-backed tests for [`LoggingMaintenanceService`] and
//! [`AnalyticsRepository::log_event`].

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use systemprompt_identifiers::{ContextId, LogId, SessionId, TraceId, UserId};
use systemprompt_logging::models::{LogEntry, LogFilter, LogLevel};
use systemprompt_logging::{AnalyticsEvent, AnalyticsRepository, LoggingMaintenanceService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn seeded_entry(module: &str, message: &str) -> LogEntry {
    let tag = uuid::Uuid::new_v4().simple().to_string();
    LogEntry {
        id: LogId::generate(),
        timestamp: Utc::now(),
        level: LogLevel::Warn,
        module: module.to_owned(),
        message: message.to_owned(),
        metadata: Some(json!({"maint": true})),
        user_id: UserId::new(format!("maint-user-{tag}")),
        session_id: SessionId::new(format!("maint-sess-{tag}")),
        task_id: None,
        trace_id: TraceId::new(format!("maint-trace-{tag}")),
        context_id: None,
        client_id: None,
    }
}

#[tokio::test]
async fn maintenance_service_reads_counts_and_cleans() {
    let Some(db) = pool().await else { return };
    let svc = LoggingMaintenanceService::new(&db).expect("maintenance service");
    let repo = systemprompt_logging::LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let module = format!("maint-mod-{}", uuid::Uuid::new_v4().simple());
    let mut old = seeded_entry(&module, "maint-old");
    old.timestamp = Utc::now() - ChronoDuration::days(45);
    let fresh = seeded_entry(&module, "maint-fresh");
    let fresh_id = fresh.id.clone();
    repo.log(old).await.unwrap();
    repo.log(fresh).await.unwrap();

    let recent = svc.get_recent_logs(200).await.unwrap();
    assert!(recent.iter().any(|e| e.module == module));

    let filter = LogFilter::new(1, 50).with_module(&module);
    let (rows, total) = svc.get_filtered_logs(&filter).await.unwrap();
    assert_eq!(total, 2);
    assert!(rows.iter().all(|r| r.module == module));

    let cutoff = Utc::now() - ChronoDuration::days(30);
    assert!(svc.count_logs_before(cutoff).await.unwrap() >= 1);
    assert!(svc.cleanup_old_logs(cutoff).await.unwrap() >= 1);

    let (remaining, remaining_total) = svc.get_filtered_logs(&filter).await.unwrap();
    assert_eq!(remaining_total, 1);
    assert_eq!(remaining[0].message, "maint-fresh");

    repo.delete_log_entry(&fresh_id).await.unwrap();
}

#[tokio::test]
async fn analytics_log_event_persists_row() {
    let Some(db) = pool().await else { return };
    let repo = AnalyticsRepository::new(&db).expect("analytics repo");

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let event_type = format!("evt-{tag}");
    let user_id = format!("an-user-{tag}");
    let session_id = format!("an-sess-{tag}");
    let write_pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&user_id)
        .bind(format!("{user_id}@test.invalid"))
        .execute(write_pool.as_ref())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, session_source) VALUES ($1, $2, 'bridge')",
    )
    .bind(&session_id)
    .bind(&user_id)
    .execute(write_pool.as_ref())
    .await
    .unwrap();

    let event = AnalyticsEvent {
        user_id: UserId::new(user_id.clone()),
        session_id: SessionId::new(session_id.clone()),
        context_id: ContextId::generate(),
        event_type: event_type.clone(),
        event_category: "test".to_owned(),
        severity: "info".to_owned(),
        endpoint: Some("/analytics".to_owned()),
        error_code: Some(0),
        response_time_ms: Some(12),
        agent_id: None,
        task_id: None,
        message: Some("analytics marker".to_owned()),
        metadata: json!({"an": 1}),
    };

    let affected = repo.log_event(&event).await.unwrap();
    assert_eq!(affected, 1);

    let row: (String, Option<String>) = sqlx::query_as(
        "SELECT event_category, message FROM analytics_events WHERE event_type = $1",
    )
    .bind(&event_type)
    .fetch_one(write_pool.as_ref())
    .await
    .unwrap();
    assert_eq!(row.0, "test");
    assert_eq!(row.1.as_deref(), Some("analytics marker"));

    sqlx::query("DELETE FROM analytics_events WHERE event_type = $1")
        .bind(&event_type)
        .execute(write_pool.as_ref())
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&user_id)
        .execute(write_pool.as_ref())
        .await
        .unwrap();
}
