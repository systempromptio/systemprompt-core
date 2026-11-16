//! `admin bridge rotate-signing-key` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use base64::Engine;
use clap::Args;
use ed25519_dalek::{SigningKey, VerifyingKey};
use systemprompt_config::SecretsService;
use systemprompt_logging::CliService;

use crate::shared::ConfirmArgs;
use systemprompt_extension::cli::CommandOutput;
use super::types::SigningKeyRotatedOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct RotateSigningKeyArgs {
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

pub(super) async fn execute(
    _args: RotateSigningKeyArgs,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let boot = crate::commands::admin::state::boot(host).await?;
    let seed = SecretsService::rotate_manifest_signing_seed(boot.loaded_profile())?;
    let key = SigningKey::from_bytes(&seed);
    let verifying: VerifyingKey = key.verifying_key();
    let pubkey_b64 = base64::engine::general_purpose::STANDARD.encode(verifying.to_bytes());

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
