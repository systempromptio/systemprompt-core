//! DB-backed tests for [`DatabaseLogService`] and
//! [`LoggingMaintenanceService`].

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use systemprompt_identifiers::{LogId, SessionId, TraceId, UserId};
use systemprompt_logging::models::{LogEntry, LogFilter, LogLevel};
use systemprompt_logging::{DatabaseLogService, LoggingMaintenanceService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::LogService;
use uuid::Uuid;

fn uniq(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

fn make_entry(module: &str, msg: &str) -> LogEntry {
    LogEntry {
        id: LogId::generate(),
        timestamp: Utc::now(),
        level: LogLevel::Info,
        module: module.to_owned(),
        message: msg.to_owned(),
        metadata: Some(json!({})),
        user_id: UserId::new(uniq("svc-user")),
        session_id: SessionId::new(uniq("svc-sess")),
        task_id: None,
        trace_id: TraceId::new(uniq("svc-trace")),
        context_id: None,
        client_id: None,
    }
}

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn database_log_service_log_get_recent_delete() {
    let Some(db) = db().await else { return };
    let svc = DatabaseLogService::new(&db).expect("ctor");
    let entry = make_entry("svc-test-mod", "svc-msg");
    let id = entry.id.clone();
    svc.log(entry).await.unwrap();

    let recent = svc.get_recent(50).await.unwrap();
    assert!(recent.iter().any(|e| e.id.as_str() == id.as_str()));

    let fetched = svc.get_by_id(id.as_str()).await.unwrap();
    assert!(fetched.is_some());

    let removed = svc.delete(id.as_str()).await.unwrap();
    assert!(removed);

    let filter = LogFilter::new(1, 10).with_module("svc-test-mod");
    let (_, _total) = svc.query(&filter).await.unwrap();
}

#[tokio::test]
async fn database_log_service_from_repository() {
    let Some(db) = db().await else { return };
    let repo = systemprompt_logging::LoggingRepository::new(&db).unwrap();
    let svc = DatabaseLogService::from_repository(repo);
    let _ = svc.repository();
}

#[tokio::test]
async fn maintenance_service_full_surface() {
    let Some(db) = db().await else { return };
    let svc = LoggingMaintenanceService::new(&db).expect("ctor");

    let _recent = svc.get_recent_logs(10).await.unwrap();

    let filter = LogFilter::new(1, 5);
    let _filtered = svc.get_filtered_logs(&filter).await.unwrap();

    let cutoff = Utc::now() - ChronoDuration::days(365);
    let _count = svc.count_logs_before(cutoff).await.unwrap();
    let _deleted = svc.cleanup_old_logs(cutoff).await.unwrap();

    LoggingMaintenanceService::vacuum();
}
