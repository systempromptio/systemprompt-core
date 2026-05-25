//! JWT signature and claims validation.

use chrono::Utc;
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_models::auth::JwtAudience;
use systemprompt_security::keys::authority;

use crate::error::{OauthError, OauthResult};
use crate::models::JwtClaims;

pub fn validate_jwt_token(
    token: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> OauthResult<JwtClaims> {
    let header = decode_header(token)
        .map_err(|e| OauthError::TokenInvalid(format!("JWT header decode failed: {e}")))?;
    if header.alg != Algorithm::RS256 {
        return Err(OauthError::TokenAlgMismatch {
            got: format!("{:?}", header.alg),
            expected: "RS256".to_owned(),
        });
    }
    let kid = header.kid.as_deref().ok_or(OauthError::TokenMissingKid)?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| OauthError::TokenInvalid(format!("signing key lookup failed: {e}")))?
        .ok_or_else(|| OauthError::TokenUnknownKid {
            kid: kid.to_owned(),
        })?;

    let mut validation = Validation::new(Algorithm::RS256);

    validation.set_issuer(&[issuer]);

    let audience_strs: Vec<&str> = audiences.iter().map(JwtAudience::as_str).collect();
    validation.set_audience(&audience_strs);

    let token_data = decode::<JwtClaims>(token, key, &validation)
        .map_err(|e| OauthError::TokenInvalid(format!("JWT validation failed: {e}")))?;

    let now = Utc::now().timestamp();

    if token_data.claims.exp < now {
        return Err(OauthError::Expired("Token has expired".to_owned()));
    }

    Ok(token_data.claims)
}
