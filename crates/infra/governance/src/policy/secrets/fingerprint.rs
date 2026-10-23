//! Keyed evidence fingerprints. The random key is drawn once per
//! [`super::SecretScanner`]; fingerprints deliberately cannot correlate
//! credentials across installations, restarts or scanner rebuilds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct FingerprintKey([u8; 32]);

impl std::fmt::Debug for FingerprintKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FingerprintKey(..)")
    }
}

impl FingerprintKey {
    pub(super) fn generate() -> Self {
        let mut key = [0; 32];
        key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        Self(key)
    }

    pub(super) fn of(&self, value: &str) -> String {
        match Hmac::<Sha256>::new_from_slice(&self.0) {
            Ok(mut mac) => {
                mac.update(b"governance-evidence-v1:");
                mac.update(value.as_bytes());
                hex::encode(mac.finalize().into_bytes())
            },
            Err(error) => {
                tracing::error!(%error, "evidence fingerprint unavailable");
                Uuid::new_v4().to_string()
            },
        }
    }
}
