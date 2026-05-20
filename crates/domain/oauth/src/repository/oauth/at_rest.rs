//! At-rest hashing for OAuth identifiers (refresh-token ids, authorisation
//! codes). The pepper is resolved once per call from the process-wide
//! [`systemprompt_config::SecretsBootstrap`] and combined with the value via
//! HMAC-SHA-256; the lowercase-hex digest is what hits the database.

use crate::error::{OauthError, OauthResult};

pub fn hash_at_rest(value: &str) -> OauthResult<String> {
    let pepper = systemprompt_config::SecretsBootstrap::oauth_at_rest_pepper()
        .map_err(|e| OauthError::Internal(format!("oauth_at_rest_pepper unavailable: {e}")))?;
    Ok(systemprompt_security::hmac_sha256_hex(
        pepper.as_bytes(),
        value.as_bytes(),
    ))
}
