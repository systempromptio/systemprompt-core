//! Signing-key loading and the CLI session lifetime for CLI commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_loader::BootContext;
use systemprompt_security::keys::SigningKeys;

pub const DEFAULT_CLI_SESSION_HOURS: i64 = 24;

pub(crate) fn load_signing_keys(boot: &BootContext) -> Result<SigningKeys> {
    let config = boot
        .try_init_config(None)
        .context("Failed to build the configuration for the signing keys")?;
    SigningKeys::resolve(&config.signing_key_path, boot.secrets())
        .context("Failed to load the signing keys")
}
