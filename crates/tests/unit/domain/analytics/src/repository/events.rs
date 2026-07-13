//! DB-backed tests for `AnalyticsEventsRepository`: single-event writes that
//! fold `content_id` / `slug` / `referrer` into `event_data`, and the
//! `find_by_content` reader that pivots on the JSON `content_id`.

use systemprompt_analytics::{AnalyticsEventsRepository, SessionRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use systemprompt_analytics::CreateSessionParams;

async fn seed_session(pool: &DbPool, session_id: &SessionId) {
    let repo = SessionRepository::new(pool).expect("session repo");
    let params = CreateSessionParams {
        session_id,
        user_id: None,
        session_source: SessionSource::Web,
        fingerprint_hash: Some("fp"),
        ip_address: None,
        user_agent: None,
        device_type: None,
        browser: None,
        os: None,
        country: None,
        region: None,
        city: None,
        preferred_locale: None,
        referrer_source: None,
        referrer_url: None,
        landing_page: None,
        entry_url: None,
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_content: None,
        utm_term: None,
        is_bot: false,
        is_ai_crawler: false,
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    };
    repo.create_session(&params).await.expect("seed session");
}

async fn cleanup(pool: &DbPool, session_id: &SessionId) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM analytics_events WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn create_event_folds_content_metadata_and_find_by_content_reads_it() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = AnalyticsEventsRepository::new(&pool).expect("repo");

    let sid = SessionId::new(format!("sess-evt-{}", Uuid::new_v4()));
    seed_session(&pool, &sid).await;
    let user = UserId::new("anon".to_owned());
    let content = ContentId::new(format!("content-{}", Uuid::new_v4()));

    let input = systemprompt_analytics::CreateAnalyticsEventInput {
        event_type: systemprompt_analytics::AnalyticsEventType::PageView,
        page_url: "/guide/x".to_owned(),
        content_id: Some(content.clone()),
        slug: Some("guide-x".to_owned()),
        referrer: Some("https://ref.example".to_owned()),
        data: None,
    };
    let created = repo
        .create_event(&sid, &user, &input)
        .await
        .expect("create event");
    assert_eq!(created.event_type, "page_view");

    let events = repo
        .find_by_content(&content, 10)
        .await
        .expect("by content");
    assert_eq!(events.len(), 1);
    let stored = &events[0];
    assert_eq!(stored.id, created.id);
    assert_eq!(
        stored.session_id.as_ref().map(SessionId::as_str),
        Some(sid.as_str())
    );
    let data = stored.event_data.as_ref().expect("event_data");
    assert_eq!(data["content_id"], serde_json::json!(content.as_str()));
    assert_eq!(data["slug"], serde_json::json!("guide-x"));
    assert_eq!(data["referrer"], serde_json::json!("https://ref.example"));

    cleanup(&pool, &sid).await;
}

#[tokio::test]
async fn find_by_content_is_empty_for_unknown_content() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = AnalyticsEventsRepository::new(&pool).expect("repo");

    let unknown = ContentId::new(format!("content-{}", Uuid::new_v4()));
    assert!(
        repo.find_by_content(&unknown, 10)
            .await
            .expect("by content")
            .is_empty()
    );
}
