//! At-rest hashing for OAuth identifiers (refresh-token ids, authorisation
//! codes). The pepper is resolved once per call from the process-wide
//! [`systemprompt_config::SecretsBootstrap`] and combined with the value via
//! HMAC-SHA-256; the lowercase-hex digest is what hits the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult;

pub(super) fn hash_at_rest(value: &str) -> OauthResult<String> {
    let pepper = systemprompt_config::SecretsBootstrap::oauth_at_rest_pepper()?;
    Ok(systemprompt_security::hmac_sha256_hex(
        pepper.as_bytes(),
        value.as_bytes(),
    )?)
}
