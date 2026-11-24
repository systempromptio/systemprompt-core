//! `admin bridge rotate-signing-key` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_extension::cli::CommandHost;
use systemprompt_logging::CliService;

use super::types::SigningKeyRotatedOutput;
use systemprompt_extension::cli::{CommandOutput, ConfirmArgs};

#[derive(Debug, Clone, Copy, Args)]
pub struct RotateSigningKeyArgs {
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

pub(super) fn execute(
    _args: RotateSigningKeyArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    let pubkey_b64 = host.signing_keys()?.rotate_manifest_key()?;

    CliService::success(&format!(
        "Manifest signing key rotated. New pubkey (base64, raw 32-byte ed25519):\n{pubkey_b64}"
    ));
    CliService::info(
        "Operators must repin this pubkey via `bridge install --pubkey <value>` before upgrading.",
    );

    let output = SigningKeyRotatedOutput {
        pubkey_b64: pubkey_b64.clone(),
        message: format!("Manifest signing key rotated; new pubkey {pubkey_b64}"),
    };

    Ok(CommandOutput::card_value(
        "Bridge Signing Key Rotated",
        &output,
    ))
}
