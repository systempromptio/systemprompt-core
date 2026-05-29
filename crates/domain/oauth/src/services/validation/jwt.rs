//! JWT signature and claims validation.
//!
//! Thin wrapper over the shared [`decode_rs256_claims`] primitive so the OAuth
//! domain validates self-issued tokens with exactly the same kid lookup, RS256
//! enforcement, and `exp`/`nbf`/issuer/audience policy as every other surface.

use systemprompt_models::auth::JwtAudience;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};

use crate::error::OauthResult;
use crate::models::JwtClaims;

pub fn validate_jwt_token(
    token: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> OauthResult<JwtClaims> {
    let policy = ValidationPolicy::issuer_scoped(issuer, audiences);
    Ok(decode_rs256_claims(token, &policy)?)
}
