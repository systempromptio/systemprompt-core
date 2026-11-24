//! `admin bridge signing-key-fingerprint`: the manifest signing key a bridge
//! is asked to trust, as the fingerprint the bridge shows.
//!
//! The fingerprint is the lowercase hex SHA-256 of the base64 `pubkey` value
//! `/v1/bridge/pubkey` serves, so an operator can read it out of band and the
//! user can compare it with the one the bridge displays before trusting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use systemprompt_extension::cli::CommandHost;

use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct SigningKeyFingerprintArgs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SigningKeyFingerprintOutput {
    pub pubkey_b64: String,
    pub fingerprint: String,
}

#[must_use]
pub fn key_fingerprint(pubkey_b64: &str) -> String {
    hex::encode(Sha256::digest(pubkey_b64.trim().as_bytes()))
}

pub(super) fn execute(
    _args: SigningKeyFingerprintArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput> {
    let pubkey_b64 = host.signing_keys()?.manifest_public_key()?;
    let output = SigningKeyFingerprintOutput {
        fingerprint: key_fingerprint(&pubkey_b64),
        pubkey_b64,
    };
    Ok(CommandOutput::card_value(
        "Bridge Manifest Signing Key",
        &output,
    ))
}
