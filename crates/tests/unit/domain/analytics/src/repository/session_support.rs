//! Shared seeding helpers for the session-repository DB tests. Sessions are
//! created with `user_id = None` to avoid the `users(id)` foreign key, and
//! `analytics_events` / `engagement_events` rows are inserted directly so the
//! behavioural read queries have data to aggregate.

use chrono::{DateTime, Duration, Utc};
use systemprompt_analytics::{CreateSessionParams, SessionRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource};
use uuid::Uuid;

pub fn unique_session_id() -> SessionId {
    SessionId::new(format!("sess-{}", Uuid::new_v4()))
}

/// Minimal session seed: no user, web source, expiry one hour out.
pub fn base_params<'a>(
    session_id: &'a SessionId,
    fingerprint_hash: Option<&'a str>,
    expires_at: DateTime<Utc>,
) -> CreateSessionParams<'a> {
    CreateSessionParams {
        session_id,
        user_id: None,
        session_source: SessionSource::Web,
        fingerprint_hash,
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
        expires_at,
    }
}

pub async fn seed_session(repo: &SessionRepository, session_id: &SessionId, fingerprint: &str) {
    let params = base_params(
        session_id,
        Some(fingerprint),
        Utc::now() + Duration::hours(1),
    );
    repo.create_session(&params).await.expect("seed session");
}

/// Insert an `analytics_events` row directly. The session FK must already
/// exist.
pub async fn insert_analytics_event(
    pool: &DbPool,
    session_id: &SessionId,
    event_type: &str,
    endpoint: Option<&str>,
    ts: DateTime<Utc>,
) {
    let id = format!("evt-{}", Uuid::new_v4());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query(
        r#"
        INSERT INTO analytics_events
            (id, user_id, session_id, event_type, event_category, severity, endpoint, timestamp)
        VALUES ($1, $2, $3, $4, 'analytics', 'info', $5, $6)
        "#,
    )
    .bind(&id)
    .bind("anon")
    .bind(session_id.as_str())
    .bind(event_type)
    .bind(endpoint)
    .bind(ts)
    .execute(p.as_ref())
    .await
    .expect("insert analytics_event");
}

/// Insert an `engagement_events` row (no session FK required).
pub async fn insert_engagement_event(pool: &DbPool, session_id: &SessionId) {
    let id = format!("eng-{}", Uuid::new_v4());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query(
        r#"
        INSERT INTO engagement_events (id, session_id, user_id, page_url)
        VALUES ($1, $2, 'anon', '/page')
        "#,
    )
    .bind(&id)
    .bind(session_id.as_str())
    .execute(p.as_ref())
    .await
    .expect("insert engagement_event");
}

pub async fn delete_session(pool: &DbPool, session_id: &SessionId) {
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("DELETE FROM analytics_events WHERE session_id = $1")
        .bind(session_id.as_str())
        .execute(p.as_ref())
        .await
        .ok();
    sqlx::query("DELETE FROM engagement_events WHERE session_id = $1")
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
