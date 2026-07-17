//! Bearer-token decode for request-context middleware.
//!
//! [`extract_user_context`] decodes via
//! [`super::validate::decode_rs256_claims`]
//! with [`ValidationPolicy::session_context`] (signature, RS256, `kid`, `exp`,
//! `nbf` + leeway, first-party `aud`), then re-derives `user_type` from
//! `scope` so a forged or mis-minted type claim cannot ride past the gate, and
//! returns the subset of claims the request-context layer consumes
//! ([`JwtUserContext`]). Issuer pinning is left to the stateful validators
//! that hold deployment config ([`crate::AuthValidationService`]); this path
//! instead binds the token to a live session and user row in the database
//! after decode.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_identifiers::{Actor, ClientId, SessionId, UserId};
use systemprompt_models::auth::{Permission, UserType};

use super::validate::{ValidationPolicy, decode_rs256_claims};
use crate::error::{AuthError, AuthResult};

#[derive(Debug, Clone)]
pub struct JwtUserContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub role: Permission,
    pub user_type: UserType,
    pub client_id: Option<ClientId>,
    pub act_chain: Vec<Actor>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub jti: String,
    pub exp: i64,
}

pub fn extract_user_context(token: &str) -> AuthResult<JwtUserContext> {
    let claims = decode_rs256_claims(token, &ValidationPolicy::session_context())?;

    let session_id = claims.session_id.ok_or(AuthError::MissingSessionId)?;
    let role = *claims.scope.first().ok_or(AuthError::MissingScope)?;
    let derived_type = UserType::from_permissions(&claims.scope);
    if derived_type != claims.user_type {
        return Err(AuthError::UserTypeMismatch {
            claimed: claims.user_type,
            derived: derived_type,
        });
    }
    let act_chain = claims
        .act
        .as_ref()
        .map(systemprompt_models::auth::ActClaim::flatten_to_chain)
        .unwrap_or_default();

    Ok(JwtUserContext {
        user_id: UserId::new(claims.sub),
        session_id,
        role,
        user_type: derived_type,
        client_id: claims.client_id,
        act_chain,
        attributes: claims.attributes,
        jti: claims.jti,
        exp: claims.exp,
    })
}
