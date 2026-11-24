//! Local session rows and signing keys for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::Duration;
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_loader::BootContext;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::AdminBootstrap;

pub const DEFAULT_CLI_SESSION_HOURS: i64 = 24;

pub fn load_signing_keys(boot: &BootContext) -> Result<SigningKeys> {
    let config = boot
        .try_init_config(None)
        .context("Failed to build the configuration for the signing keys")?;
    SigningKeys::resolve(&config.signing_key_path, boot.secrets())
        .context("Failed to load the signing keys")
}

pub async fn create_local_session_row(
    admins: &dyn AdminBootstrap,
    user: &UserId,
    ttl: Duration,
) -> Result<SessionId> {
    admins
        .open_session(user, ttl, SessionSource::Cli)
        .await
        .context("Failed to insert CLI session row")
}
