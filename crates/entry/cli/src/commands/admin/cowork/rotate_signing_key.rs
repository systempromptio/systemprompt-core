use anyhow::Result;
use base64::Engine;
use clap::Args;
use ed25519_dalek::{SigningKey, VerifyingKey};
use systemprompt_models::SecretsBootstrap;

use super::types::SigningKeyRotatedOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct RotateSigningKeyArgs;

#[allow(clippy::print_stdout, clippy::unused_async)]
pub async fn execute(
    _args: RotateSigningKeyArgs,
    _config: &CliConfig,
) -> Result<CommandResult<SigningKeyRotatedOutput>> {
    SecretsBootstrap::try_init()?;

    let seed = SecretsBootstrap::rotate_manifest_signing_seed()?;
    let key = SigningKey::from_bytes(&seed);
    let verifying: VerifyingKey = key.verifying_key();
    let pubkey_b64 = base64::engine::general_purpose::STANDARD.encode(verifying.to_bytes());

    println!(
        "Manifest signing key rotated. New pubkey (base64, raw 32-byte ed25519):\n{pubkey_b64}"
    );
    println!(
        "Operators must repin this pubkey via `cowork install --pubkey <value>` before upgrading."
    );

    let output = SigningKeyRotatedOutput {
        pubkey_b64: pubkey_b64.clone(),
        message: format!("Manifest signing key rotated; new pubkey {pubkey_b64}"),
    };

    Ok(CommandResult::text(output).with_title("Cowork Signing Key Rotated"))
}
