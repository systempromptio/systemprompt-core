//! At-rest hashing for secrets that must be looked up by exact match but
//! must not survive a database read.
//!
//! The deployment's `oauth_at_rest_pepper` is a process-resident HMAC key.
//! Refresh-token identifiers and authorisation codes are stored as the
//! lowercase-hex HMAC-SHA-256 of the raw value under that key, so a leaked
//! database backup or replica snapshot yields opaque digests rather than
//! live credentials.
//!
//! Rotation is out of scope here: rolling the pepper invalidates every
//! row hashed under the old key. The schema reserves no `pepper_version`
//! column today; a future migration would add one if graceful rotation is
//! required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[expect(
    clippy::expect_used,
    reason = "HMAC-SHA256 accepts any key length by construction; new_from_slice cannot fail here"
)]
pub fn hmac_sha256(pepper: &[u8], value: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(pepper).expect("HMAC accepts any key length");
    mac.update(value);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

pub fn hmac_sha256_hex(pepper: &[u8], value: &[u8]) -> String {
    hex::encode(hmac_sha256(pepper, value))
}
