//! Bundle signing seeds: where they come from and what they identify.
//!
//! A bundle signing key is an ed25519 seed owned by the editing repository,
//! never the instance manifest seed. It is supplied as a file holding the
//! base64 seed, or as `env:VAR` naming an environment variable, so CI can
//! keep it in its own secret store and never write it to the workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine;
use rand::{Rng, rng};
use systemprompt_security::manifest_signing::{key_id_for_pubkey, pubkey_b64_from_seed};

pub const SEED_BYTES: usize = 32;
pub const ENV_PREFIX: &str = "env:";

#[derive(Debug, Clone)]
pub struct BundleSigningKey {
    pub seed: [u8; SEED_BYTES],
    pub public_key: String,
    pub key_id: String,
}

impl BundleSigningKey {
    #[must_use]
    pub fn from_seed(seed: [u8; SEED_BYTES]) -> Self {
        let public_key = pubkey_b64_from_seed(&seed);
        let key_id = key_id_for_pubkey(&public_key);
        Self {
            seed,
            public_key,
            key_id,
        }
    }

    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0u8; SEED_BYTES];
        rng().fill_bytes(&mut seed);
        Self::from_seed(seed)
    }

    #[must_use]
    pub fn seed_b64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.seed)
    }
}

pub fn load_signing_key(reference: &str) -> Result<BundleSigningKey> {
    let encoded = if let Some(var) = reference.strip_prefix(ENV_PREFIX) {
        std::env::var(var).with_context(|| format!("Signing key variable {var} is not set"))?
    } else {
        std::fs::read_to_string(Path::new(reference))
            .with_context(|| format!("Failed to read signing key {reference}"))?
    };
    decode_seed(encoded.trim())
}

pub fn decode_seed(encoded: &str) -> Result<BundleSigningKey> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("Signing key is not valid base64")?;
    let seed: [u8; SEED_BYTES] = raw.as_slice().try_into().map_or_else(
        |_e| {
            bail!(
                "Signing key must decode to {SEED_BYTES} bytes, got {}",
                raw.len()
            )
        },
        Ok,
    )?;
    Ok(BundleSigningKey::from_seed(seed))
}
