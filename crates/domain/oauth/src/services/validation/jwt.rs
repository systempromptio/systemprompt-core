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
        .map_err(|e| OauthError::Token(format!("JWT header decode failed: {e}")))?;
    if header.alg != Algorithm::RS256 {
        return Err(OauthError::Token("JWT must be RS256-signed".to_string()));
    }
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| OauthError::Token("JWT is missing `kid` header".to_string()))?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| OauthError::Token(format!("signing key lookup failed: {e}")))?
        .ok_or_else(|| OauthError::Token(format!("unknown `kid` `{kid}`")))?;

    let mut validation = Validation::new(Algorithm::RS256);

    validation.set_issuer(&[issuer]);

    let audience_strs: Vec<&str> = audiences.iter().map(JwtAudience::as_str).collect();
    validation.set_audience(&audience_strs);

    let token_data = decode::<JwtClaims>(token, key, &validation)
        .map_err(|e| OauthError::Token(format!("JWT validation failed: {e}")))?;

    let now = Utc::now().timestamp();

    if token_data.claims.exp < now {
        return Err(OauthError::Token("Token has expired".to_string()));
    }

    Ok(token_data.claims)
}
