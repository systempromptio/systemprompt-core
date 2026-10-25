//! Local session and JWT minting for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Duration;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_loader::BootContext;
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{SessionAnalytics, SessionProvider, UserProvider};
use systemprompt_users::{UserRepository, UserService};

pub const DEFAULT_CLI_SESSION_HOURS: i64 = 24;

pub fn load_signing_keys(boot: &BootContext) -> Result<SigningKeys> {
    let config = boot
        .try_init_config(None)
        .context("Failed to build the configuration for the signing keys")?;
    SigningKeys::resolve(&config.signing_key_path, boot.secrets())
        .context("Failed to load the signing keys")
}

pub async fn create_local_session_row(
    db_pool: &DbPool,
    user: &UserId,
    ttl: Duration,
    keys: SigningKeys,
    boot: &BootContext,
) -> Result<SessionId> {
    let config = Arc::clone(
        boot.try_init_config(None)
            .context("Failed to build the configuration for the session service")?,
    );
    let sessions: Arc<dyn SessionProvider> =
        Arc::new(systemprompt_users::SessionRepository::new(db_pool));
    let user_repository = Arc::new(UserRepository::new(db_pool));
    let users: Arc<dyn UserProvider> = Arc::new(UserService::new(user_repository));

    SessionCreationService::new(sessions, users, keys, config)
        .create_authenticated_session_with_ttl(
            user,
            &SessionAnalytics::default(),
            SessionSource::Cli,
            ttl,
        )
        .await
        .context("Failed to insert CLI session row")
}
