//! A per-process fingerprint of the configuration a replica is running with.
//!
//! During a rollout, or after a secret rotation, replicas can run different
//! configuration; the fingerprint makes that visible from outside without
//! exposing anything sensitive. `profile_sha` hashes the loaded profile,
//! `secret_keys_sha` hashes the sorted *names* of the loaded secrets (never a
//! value, never a hash of a value), `secrets_generation` counts accepted
//! reloads, and `services_sha` is the composed services-tree hash when the
//! tree was fetched from bundles. Digests are truncated to
//! [`FINGERPRINT_HEX_LEN`] hex characters: they identify a configuration, they
//! do not authenticate one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use systemprompt_manifest::profile::Profile;

use crate::bootstrap::{ProfileBootstrap, SecretsBootstrap};

pub const FINGERPRINT_HEX_LEN: usize = 16;

/// What one replica has loaded, safe to publish on probes and metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigFingerprint {
    pub profile_sha: String,
    pub secret_keys_sha: String,
    pub secrets_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_last_reload_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services_sha: Option<String>,
}

impl ConfigFingerprint {
    pub fn compute(
        profile: &Profile,
        secret_keys: &[String],
        secrets_generation: u64,
        services_sha: Option<String>,
    ) -> Result<Self, serde_json::Error> {
        let profile_bytes = serde_json::to_vec(profile)?;
        let mut names: Vec<&str> = secret_keys.iter().map(String::as_str).collect();
        names.sort_unstable();
        names.dedup();
        Ok(Self {
            profile_sha: short_digest(&profile_bytes),
            secret_keys_sha: short_digest(names.join("\n").as_bytes()),
            secrets_generation,
            secrets_last_reload_at: None,
            services_sha: services_sha.map(|hash| hash.chars().take(FINGERPRINT_HEX_LEN).collect()),
        })
    }

    #[must_use]
    pub fn current(services_sha: Option<String>) -> Option<Self> {
        let profile = ProfileBootstrap::get().ok()?;
        let keys = SecretsBootstrap::key_names()
            .inspect_err(|error| tracing::debug!(error = %error, "no secrets for fingerprint"))
            .unwrap_or_default();
        let mut fingerprint =
            Self::compute(profile, &keys, SecretsBootstrap::generation(), services_sha)
                .inspect_err(|error| {
                    tracing::warn!(error = %error, "profile could not be fingerprinted");
                })
                .ok()?;
        fingerprint.secrets_last_reload_at =
            SecretsBootstrap::status().and_then(|status| status.last_reload_at);
        Some(fingerprint)
    }
}

fn short_digest(bytes: &[u8]) -> String {
    let mut hex = hex::encode(Sha256::digest(bytes));
    hex.truncate(FINGERPRINT_HEX_LEN);
    hex
}
