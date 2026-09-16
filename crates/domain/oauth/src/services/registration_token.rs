//! RFC 7592 registration access tokens.
//!
//! The token is opaque and high-entropy, so it is stored as a plain SHA-256
//! digest (no work factor needed) and compared in constant time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const REGISTRATION_TOKEN_PREFIX: &str = "reg_";

#[must_use]
pub fn generate_registration_token() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    format!("{REGISTRATION_TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(buf))
}

#[must_use]
pub fn hash_registration_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[must_use]
pub fn verify_registration_token(token: &str, stored_hash: Option<&str>) -> bool {
    let Some(stored) = stored_hash else {
        return false;
    };
    let presented = hash_registration_token(token);
    presented.as_bytes().ct_eq(stored.as_bytes()).into()
}
