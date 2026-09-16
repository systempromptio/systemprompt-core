//! DB-backed unit tests for [`LoggingRepository`] and [`AnalyticsRepository`].
//!
//! These hit the `logs` and `analytics_events` tables on the per-track Postgres
//! database. Each test owns isolated row ids and cleans up after itself.

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use systemprompt_identifiers::{LogId, SessionId, TraceId, UserId};
use systemprompt_logging::models::{LogEntry, LogFilter, LogLevel};
use systemprompt_logging::{AnalyticsRepository, DatabaseLogService, LoggingRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn analytics_ingestion_preserves_payloads_and_rejects_batches_atomically() {
    use systemprompt_traits::analytics_events::{AnalyticsEventRecord, AnalyticsEventStore};

    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pool = db.write_pool_arc().unwrap();
    let session_id = SessionId::new(unique_id("event-store"));
    sqlx::query("INSERT INTO user_sessions (session_id, session_source) VALUES ($1, 'web')")
        .bind(session_id.as_str())
        .execute(pool.as_ref())
        .await
        .unwrap();
    let unavailable_replica =
        sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").unwrap();
    unavailable_replica.close().await;
    let split_db = std::sync::Arc::new(systemprompt_database::Database::from_pools(
        std::sync::Arc::new(unavailable_replica),
        Some(pool.clone()),
    ));
    let store = AnalyticsRepository::new(&split_db).unwrap();
    assert!(!store.has_analytics_events(&session_id).await.unwrap());
    store.persist_events(&[]).await.unwrap();
    let first = AnalyticsEventRecord {
        id: format!("evt_{}", uuid::Uuid::new_v4()),
        user_id: UserId::new("anon"),
        session_id: session_id.clone(),
        event_type: "page_view".to_owned(),
        event_category: "navigation".to_owned(),
        page_url: "/original".to_owned(),
        event_data: json!({"content_id": "content-1", "nested": {"value": 7}}),
    };
    store
        .persist_events(std::slice::from_ref(&first))
        .await
        .unwrap();
    let stored: (String, String, String, serde_json::Value) = sqlx::query_as(
        "SELECT event_type, severity, endpoint, event_data FROM analytics_events WHERE id = $1",
    )
    .bind(&first.id)
    .fetch_one(pool.as_ref())
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            first.event_type.clone(),
            "info".to_owned(),
            first.page_url.clone(),
            first.event_data.clone()
        )
    );

    let mut second = first.clone();
    second.id = format!("evt_{}", uuid::Uuid::new_v4());
    second.page_url = "/second".to_owned();
    second.event_data = json!([1, "unmodified"]);
    assert!(
        store
            .persist_events(&[second.clone(), first.clone()])
            .await
            .is_err()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM analytics_events WHERE session_id = $1")
            .bind(session_id.as_str())
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(count, 1);
    assert!(store.has_analytics_events(&session_id).await.unwrap());
    assert_eq!(
        store.get_endpoint_sequence(&session_id).await.unwrap(),
        vec!["/original"]
    );
    assert_eq!(
        store
            .get_request_timestamps(&session_id)
            .await
            .unwrap()
            .len(),
        1
    );

    store
        .persist_events(std::slice::from_ref(&second))
        .await
        .unwrap();
    let payload: serde_json::Value =
        sqlx::query_scalar("SELECT event_data FROM analytics_events WHERE id = $1")
            .bind(&second.id)
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
    assert_eq!(payload, second.event_data);
    sqlx::query("DELETE FROM analytics_events WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(pool.as_ref())
        .await
        .unwrap();
    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(pool.as_ref())
        .await
        .unwrap();
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
        instance_id: None,
    }
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    drop(LoggingRepository::new(&db).expect("repo new"));
}

#[tokio::test]
async fn log_with_database_persists_then_fetch_by_id() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();
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
async fn log_rejects_invalid_entry() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();
    let actor = make_actor("invalid");
    let mut bad = make_entry("ok-mod", "ok-msg", &actor);
    bad.module = String::new();
    let err = repo.log(bad).await.unwrap_err();
    assert!(format!("{err:?}").contains("Module") || format!("{err:?}").contains("Empty"));
}

#[tokio::test]
async fn get_recent_logs_returns_inserted_rows() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();

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

    let deleted = repo.delete_log_entries(&ids).await.unwrap();
    assert_eq!(deleted, ids.len() as u64, "all seeded rows must be deleted");
}

#[tokio::test]
async fn get_logs_paginated_with_filter() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();

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
    for id in &ids {
        assert!(
            rows.iter().any(|r| r.id == *id),
            "seeded row {id} must appear on the filtered first page"
        );
    }

    repo.delete_log_entries(&ids).await.unwrap();
}

#[tokio::test]
async fn get_logs_by_module_patterns() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();

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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();

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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let repo = LoggingRepository::new(&db).unwrap();

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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let svc = DatabaseLogService::new(&db).expect("ctor");
    let _r = svc.repository();
}

#[tokio::test]
async fn analytics_repository_constructs() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let _repo = AnalyticsRepository::new(&db).expect("repo");
}
