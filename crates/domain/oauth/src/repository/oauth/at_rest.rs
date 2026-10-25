//! At-rest hashing for OAuth identifiers (refresh-token ids, authorisation
//! codes). The pepper is captured once when the repository is built and
//! combined with the value via HMAC-SHA-256; the lowercase-hex digest is what
//! hits the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;

impl OAuthRepository {
    pub(super) fn hash_at_rest(&self, value: &str) -> OauthResult<String> {
        Ok(systemprompt_security::hmac_sha256_hex(
            self.pepper.as_bytes(),
            value.as_bytes(),
        )?)
    }
}
