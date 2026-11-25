//! Decoding a member's scoped token: the RS256 `kid` lookup over the
//! deployment's verification keys, the deployment issuer, exactly one
//! audience, and `exp`/`nbf` with no leeway.
//!
//! The caller (the member's conferred minter) pins the audience to one its
//! member declared, which admission keeps out of the core audiences, so this
//! decode never accepts a core token and no core verifier accepts a scoped one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use serde::Deserialize;

use super::validate::reject_critical_header;
use crate::error::{AuthError, AuthResult};
use crate::keys::VerificationKeyring;

/// The registered and scoped claims a member's minter signs.
#[derive(Debug, Clone, Deserialize)]
pub struct ScopedTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: Vec<String>,
    pub scope: String,
    pub azp: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn decode_scoped_token(
    keys: &VerificationKeyring,
    issuer: &str,
    audience: &str,
    token: &str,
) -> AuthResult<ScopedTokenClaims> {
    let header = decode_header(token).map_err(AuthError::InvalidToken)?;
    reject_critical_header(&header).map_err(AuthError::InvalidToken)?;
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::UnsupportedAlgorithm {
            got: format!("{:?}", header.alg),
        });
    }
    let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
    let key = keys
        .decoding_key_for_kid(kid)
        .ok_or_else(|| AuthError::UnknownKid {
            kid: kid.to_owned(),
        })?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 0;
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    let claims = decode::<ScopedTokenClaims>(token, key, &validation)
        .map(|data| data.claims)
        .map_err(AuthError::InvalidToken)?;
    if claims.aud.len() != 1 {
        return Err(AuthError::InvalidToken(
            jsonwebtoken::errors::ErrorKind::InvalidAudience.into(),
        ));
    }
    Ok(claims)
}
