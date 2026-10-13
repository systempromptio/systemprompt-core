//! The RS256 app JWT a GitHub App signs to ask for an installation token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;

use crate::credential::CredentialError;

// Why: GitHub refuses an app JWT whose `exp` is more than ten minutes ahead.
const APP_JWT_TTL_SECS: u64 = 600;

// Why: GitHub recommends back-dating `iat` by sixty seconds to absorb clock
// drift between this host and GitHub.
const CLOCK_SKEW_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppJwtClaims {
    pub iat: u64,
    pub exp: u64,
    pub iss: String,
}

#[must_use]
pub fn app_jwt_claims(app_id: &str, now: u64) -> AppJwtClaims {
    AppJwtClaims {
        iat: now.saturating_sub(CLOCK_SKEW_SECS),
        exp: now + APP_JWT_TTL_SECS,
        iss: app_id.to_owned(),
    }
}

pub fn sign_app_jwt(app_id: &str, private_key_pem: &str) -> Result<String, CredentialError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(CredentialError::Clock)?
        .as_secs();
    let encoding = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(CredentialError::SigningKey)?;
    jsonwebtoken::encode(
        &Header::new(Algorithm::RS256),
        &app_jwt_claims(app_id, now),
        &encoding,
    )
    .map_err(CredentialError::Sign)
}
