//! Local session and JWT minting for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Duration;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_manifest::Config;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{SessionAnalytics, SessionProvider, UserProvider};
use systemprompt_users::{UserRepository, UserService};

pub const DEFAULT_CLI_SESSION_HOURS: i64 = 24;

pub fn load_signing_keys() -> Result<SigningKeys> {
    let signing_key_path = Config::get()
        .map(|config| config.signing_key_path.clone())
        .unwrap_or_default();
    SigningKeys::resolve(&signing_key_path).context("Failed to load the signing keys")
}

pub async fn create_local_session_row(
    db_pool: &DbPool,
    user: &UserId,
    ttl: Duration,
    keys: SigningKeys,
) -> Result<SessionId> {
    let sessions: Arc<dyn SessionProvider> =
        Arc::new(systemprompt_users::SessionRepository::new(db_pool));
    let user_repository = Arc::new(UserRepository::new(db_pool));
    let users: Arc<dyn UserProvider> = Arc::new(UserService::new(user_repository));

    SessionCreationService::new(sessions, users, keys)
        .create_authenticated_session_with_ttl(
            user,
            &SessionAnalytics::default(),
            SessionSource::Cli,
            ttl,
        )
        .await
        .context("Failed to insert CLI session row")
}
