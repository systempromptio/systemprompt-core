//! Keyed evidence fingerprints. The random key is process-local; fingerprints
//! deliberately cannot correlate credentials across installations or restarts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::sync::LazyLock;
use uuid::Uuid;

static KEY: LazyLock<[u8; 32]> = LazyLock::new(|| {
    let mut key = [0; 32];
    key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    key
});

pub(super) fn of(value: &str) -> String {
    match Hmac::<Sha256>::new_from_slice(KEY.as_slice()) {
        Ok(mut mac) => {
            mac.update(b"governance-evidence-v1:");
            mac.update(value.as_bytes());
            mac.finalize()
                .into_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        },
        Err(error) => {
            tracing::error!(%error, "evidence fingerprint unavailable");
            Uuid::new_v4().to_string()
        },
    }
}
