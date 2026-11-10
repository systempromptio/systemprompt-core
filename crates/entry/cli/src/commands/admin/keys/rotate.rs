//! `admin keys rotate` command: replace the RS256 token key with overlap.
//!
//! The new key becomes `signing_key_pem` in the profile's secrets file and
//! the key it replaces is kept, public half only, as the retired verification
//! key until `--overlap-secs` from now (default: the access-token lifetime).
//! Tokens already issued keep verifying through that window, so a rotation
//! does not sign every user and bridge out at once. The running server adopts
//! the new pair on restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Args;
use serde::{Deserialize, Serialize};
use systemprompt_config::SecretsService;
use systemprompt_logging::CliService;
use systemprompt_security::keys::rotation_from;

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct RotateArgs {
    #[arg(
        long,
        help = "Seconds the replaced key keeps verifying tokens (default: the access-token lifetime)"
    )]
    pub overlap_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SigningKeyRotationOutput {
    pub active_kid: String,
    pub retired_kid: String,
    pub retired_until: chrono::DateTime<Utc>,
}

pub(super) fn execute(args: RotateArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let profile = ctx.loaded_profile()?;
    let boot = ctx.boot()?;
    let access_lifetime = profile.profile().security.access_token_expiration;
    let overlap_secs = args.overlap_secs.unwrap_or(access_lifetime);
    if overlap_secs <= 0 {
        anyhow::bail!("--overlap-secs must be positive (got {overlap_secs})");
    }
    if overlap_secs < access_lifetime {
        CliService::warning(&format!(
            "--overlap-secs {overlap_secs} is shorter than the access-token lifetime \
             ({access_lifetime}s): tokens issued just before the rotation stop verifying early"
        ));
    }

    let current = crate::session::api::load_signing_keys(boot)?;
    let retired_until = Utc::now() + Duration::seconds(overlap_secs);
    let (next, rotation) = rotation_from(current.signing_key(), retired_until)
        .context("Failed to generate the replacement signing key")?;
    SecretsService::persist_signing_key_rotation(profile, &rotation)
        .context("Failed to write the rotated signing key to the secrets file")?;

    let output = SigningKeyRotationOutput {
        active_kid: next.kid().to_owned(),
        retired_kid: current.active_kid().to_owned(),
        retired_until,
    };
    CliService::success(&format!(
        "Signing key rotated: active kid {}, retired kid {} verifies until {}",
        output.active_kid,
        output.retired_kid,
        output.retired_until.to_rfc3339()
    ));
    CliService::info(
        "Restart the server, and redeploy cloud replicas, to start signing with the new key.",
    );
    Ok(CommandOutput::card_value("Signing Key Rotated", &output))
}
