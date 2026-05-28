//! DB-backed unit tests for [`LoggingRepository`] and [`AnalyticsRepository`].
//!
//! These hit the `logs` and `analytics_events` tables on the per-track Postgres
//! database. Each test owns isolated row ids and cleans up after itself.

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use systemprompt_identifiers::{LogId, SessionId, TraceId, UserId};
use systemprompt_logging::models::{LogEntry, LogFilter, LogLevel};
use systemprompt_logging::{
    AnalyticsEvent, AnalyticsRepository, DatabaseLogService, LoggingRepository,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn make_actor(prefix: &str) -> (UserId, SessionId, TraceId) {
    (
        UserId::new(unique_id(&format!("{prefix}-user"))),
        SessionId::new(unique_id(&format!("{prefix}-sess"))),
        TraceId::new(unique_id(&format!("{prefix}-trace"))),
    )
}

fn make_entry(module: &str, msg: &str, actor: &(UserId, SessionId, TraceId)) -> LogEntry {
    LogEntry {
        id: LogId::generate(),
        timestamp: Utc::now(),
        level: LogLevel::Info,
        module: module.to_owned(),
        message: msg.to_owned(),
        metadata: Some(json!({"k": "v"})),
        user_id: actor.0.clone(),
        session_id: actor.1.clone(),
        task_id: None,
        trace_id: actor.2.clone(),
        context_id: None,
        client_id: None,
    }
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = pool().await else { return };
    let _ = LoggingRepository::new(&db).expect("repo new");
}

#[tokio::test]
async fn log_with_database_persists_then_fetch_by_id() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);
    let actor = make_actor("persist");
    let entry = make_entry("repo-test", "persisted row", &actor);
    let id = entry.id.clone();
    repo.log(entry.clone()).await.unwrap();

    let fetched = repo.get_by_id(&id).await.unwrap().expect("row");
    assert_eq!(fetched.module, "repo-test");
    assert_eq!(fetched.message, "persisted row");

    let deleted = repo.delete_log_entry(&id).await.unwrap();
    assert!(deleted);
    let again = repo.get_by_id(&id).await.unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn log_terminal_only_does_not_persist() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(false);
    let actor = make_actor("terminal");
    let entry = make_entry("term-only", "msg", &actor);
    let id = entry.id.clone();
    repo.log(entry).await.unwrap();
    assert!(repo.get_by_id(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn log_rejects_invalid_entry() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);
    let actor = make_actor("invalid");
    let mut bad = make_entry("ok-mod", "ok-msg", &actor);
    bad.module = String::new();
    let err = repo.log(bad).await.unwrap_err();
    assert!(format!("{err:?}").contains("Module") || format!("{err:?}").contains("Empty"));
}

#[tokio::test]
async fn get_recent_logs_returns_inserted_rows() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let actor = make_actor("recent");
    let mut ids = Vec::new();
    for i in 0..3 {
        let e = make_entry("recent-mod", &format!("msg-{i}"), &actor);
        ids.push(e.id.clone());
        repo.log(e).await.unwrap();
    }

    let recent = repo.get_recent_logs(100).await.unwrap();
    let found = recent.iter().filter(|e| e.module == "recent-mod").count();
    assert!(found >= 3);

    let _ = repo.delete_log_entries(&ids).await.unwrap();
}

#[tokio::test]
async fn get_logs_paginated_with_filter() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let actor = make_actor("paginated");
    let mut ids = Vec::new();
    for i in 0..2 {
        let mut e = make_entry("paginated-mod", &format!("p-{i}"), &actor);
        e.level = LogLevel::Warn;
        ids.push(e.id.clone());
        repo.log(e).await.unwrap();
    }

    let filter = LogFilter::new(1, 10)
        .with_level("WARN")
        .with_module("paginated-mod");
    let (rows, total) = repo.get_logs_paginated(&filter).await.unwrap();
    assert!(total >= 2);
    assert!(!rows.is_empty());

    repo.delete_log_entries(&ids).await.unwrap();
}

#[tokio::test]
async fn get_logs_by_module_patterns() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let actor = make_actor("by-mod");
    let e = make_entry("module-pattern-test", "pat", &actor);
    let id = e.id.clone();
    repo.log(e).await.unwrap();

    let rows = repo
        .get_logs_by_module_patterns(&["module-pattern-test".to_owned()], 10)
        .await
        .unwrap();
    assert!(rows.iter().any(|r| r.id.as_str() == id.as_str()));

    repo.delete_log_entry(&id).await.unwrap();
}

#[tokio::test]
async fn update_log_entry_updates_message() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let actor = make_actor("update");
    let mut e = make_entry("update-mod", "old", &actor);
    let id = e.id.clone();
    repo.log(e.clone()).await.unwrap();

    e.message = "new".to_owned();
    let updated = repo.update_log_entry(&id, &e).await.unwrap();
    assert!(updated);
    let f = repo.get_by_id(&id).await.unwrap().unwrap();
    assert_eq!(f.message, "new");

    repo.delete_log_entry(&id).await.unwrap();
}

#[tokio::test]
async fn cleanup_old_logs_removes_old_rows() {
    let Some(db) = pool().await else { return };
    let repo = LoggingRepository::new(&db)
        .unwrap()
        .with_terminal(false)
        .with_database(true);

    let actor = make_actor("cleanup");
    let mut e = make_entry("cleanup-mod", "old-msg", &actor);
    e.timestamp = Utc::now() - ChronoDuration::days(30);
    let id = e.id.clone();
    repo.log(e).await.unwrap();

    let cutoff = Utc::now() - ChronoDuration::days(1);
    let count = repo.count_logs_before(cutoff).await.unwrap();
    assert!(count >= 1);

    let deleted = repo.cleanup_old_logs(cutoff).await.unwrap();
    assert!(deleted >= 1);
    assert!(repo.get_by_id(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn database_log_service_construction() {
    let Some(db) = pool().await else { return };
    let svc = DatabaseLogService::new(&db).expect("ctor");
    let _r = svc.repository();
}

#[tokio::test]
async fn analytics_repository_constructs() {
    let Some(db) = pool().await else { return };
    let _repo = AnalyticsRepository::new(&db).expect("repo");
}

#[test]
fn analytics_event_struct_constructs() {
    let event = AnalyticsEvent {
        user_id: UserId::new("u"),
        session_id: SessionId::new("s"),
        context_id: systemprompt_identifiers::ContextId::generate(),
        event_type: "et".to_owned(),
        event_category: "ec".to_owned(),
        severity: "info".to_owned(),
        endpoint: Some("/x".to_owned()),
        error_code: Some(1),
        response_time_ms: Some(42),
        agent_id: None,
        task_id: None,
        message: Some("m".to_owned()),
        metadata: json!({"k": "v"}),
    };
    let cloned = event.clone();
    assert_eq!(cloned.event_type, "et");
    let _ = format!("{event:?}");
}
