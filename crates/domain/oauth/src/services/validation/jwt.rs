//! JWT signature and claims validation.
//!
//! Thin wrapper over [`TokenAuthority::verify_bearer`] so the OAuth
//! domain validates self-issued tokens with exactly the same kid lookup, RS256
//! enforcement, and `exp`/`nbf`/issuer/audience policy as every other surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::{JwtAudience, TokenAuthority};

use crate::error::OauthResult;
use crate::models::JwtClaims;

pub fn validate_jwt_token(
    authority: &dyn TokenAuthority,
    token: &str,
    audiences: &[JwtAudience],
) -> OauthResult<JwtClaims> {
    Ok(authority.verify_bearer(token, audiences)?)
}
