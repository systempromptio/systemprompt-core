//! Local session and JWT minting for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Duration;
use systemprompt_analytics::{CreateSessionParams, SessionRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use uuid::Uuid;

// Why: the public `POST /oauth/session` endpoint must not accept a
// caller-supplied `user_id` — doing so allows arbitrary admin-JWT issuance
// against any known user UUID on a public route. The CLI is colocated with
// the database and holds the JWT signing secret, so it mints session rows
// (and the JWTs above) locally instead of round-tripping through the public
// HTTP endpoint.
pub async fn create_local_session_row(db_pool: &DbPool, user: &UserId) -> Result<SessionId> {
    let session_repo =
        SessionRepository::new(db_pool).context("Failed to construct session repository")?;

    let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));
    let expires_at = chrono::Utc::now() + Duration::hours(24);

    let params = CreateSessionParams {
        session_id: &session_id,
        user_id: Some(user),
        session_source: SessionSource::Cli,
        fingerprint_hash: None,
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
    };

    session_repo
        .create_session(&params)
        .await
        .context("Failed to insert CLI session row")?;

    Ok(session_id)
}
