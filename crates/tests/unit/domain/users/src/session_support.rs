//! Shared seeding helpers for the session-repository DB tests. Sessions are
//! created with `user_id = None` to avoid the `users(id)` foreign key, and
//! `analytics_events` / `engagement_events` rows are inserted directly so the
//! behavioural read queries have data to aggregate.

use chrono::{DateTime, Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource};
use systemprompt_traits::session_store::{
    CreateSessionParams, SessionAttribution, SessionIdentity, SessionNetwork,
};
use systemprompt_users::SessionRepository;
use uuid::Uuid;

pub fn unique_session_id() -> SessionId {
    SessionId::new(format!("sess-{}", Uuid::new_v4()))
}

// Minimal session seed: no user, web source, expiry one hour out.
pub fn base_params<'a>(
    session_id: &'a SessionId,
    fingerprint_hash: Option<&'a str>,
    expires_at: DateTime<Utc>,
) -> CreateSessionParams<'a> {
    CreateSessionParams {
        identity: SessionIdentity {
            session_id,
            user_id: None,
            session_source: SessionSource::Web,
            fingerprint_hash,
        },
        network: SessionNetwork::default(),
        attribution: SessionAttribution::default(),
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

// exist.


pub async fn delete_session(pool: &DbPool, session_id: &SessionId) {
    let p = pool.pool();
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
