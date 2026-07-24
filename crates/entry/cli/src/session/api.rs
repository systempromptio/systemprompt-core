//! Local session and JWT minting for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Duration;
use std::sync::Arc;
use systemprompt_analytics::AnalyticsService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::{AnalyticsProvider, SessionAnalytics, UserProvider};
use systemprompt_users::UserService;

/// Lifetime of a CLI session row and of the admin token that names it. The two
/// must agree, or the operator sees a mid-session 401.
pub const DEFAULT_CLI_SESSION_HOURS: i64 = 24;

// Why: the public `POST /oauth/session` endpoint must not accept a
// caller-supplied `user_id` — doing so allows arbitrary admin-JWT issuance
// against any known user UUID on a public route. The CLI is colocated with
// the database and holds the JWT signing secret, so it mints session rows
// (and the JWTs above) locally instead of round-tripping through the public
// HTTP endpoint. It goes through `SessionCreationService` rather than the
// repository so every `user_sessions` row in the deployment is written by one
// code path, whatever minted it.
pub async fn create_local_session_row(
    db_pool: &DbPool,
    user: &UserId,
    ttl: Duration,
) -> Result<SessionId> {
    let analytics: Arc<dyn AnalyticsProvider> = Arc::new(
        AnalyticsService::new(db_pool, None, None)
            .context("Failed to construct analytics service")?,
    );
    let users: Arc<dyn UserProvider> =
        Arc::new(UserService::new(db_pool).context("Failed to construct user service")?);

    SessionCreationService::new(analytics, users)
        .create_authenticated_session_with_ttl(
            user,
            &SessionAnalytics::default(),
            SessionSource::Cli,
            ttl,
        )
        .await
        .context("Failed to insert CLI session row")
}
