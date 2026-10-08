//! Bearer-token decode for request-context middleware.
//!
//! [`extract_user_context`] decodes via
//! [`super::validate::decode_session_claims`] with
//! [`ValidationPolicy::session_context`] (signature, RS256, `kid`, `exp`,
//! `nbf` + leeway, the deployment issuer, first-party `aud`, act-chain depth,
//! `user_type` re-derived from `scope`) — the same checks the A2A
//! [`crate::AuthValidationService`] applies — and returns the subset of claims
//! the request-context layer consumes ([`JwtUserContext`]). The caller then
//! binds the token to a live session and user row in the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_identifiers::{AccessTokenId, Actor, ClientId, SessionId, UserId};
use systemprompt_models::auth::{Permission, UserType};

use super::validate::{ValidationPolicy, decode_session_claims};
use crate::error::{AuthError, AuthResult};

#[derive(Debug, Clone)]
pub struct JwtUserContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub role: Permission,
    pub user_type: UserType,
    pub client_id: Option<ClientId>,
    pub act_chain: Vec<Actor>,
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub jti: Option<AccessTokenId>,
    pub exp: i64,
}

pub fn extract_user_context(token: &str, issuer: &str) -> AuthResult<JwtUserContext> {
    let claims = decode_session_claims(token, &ValidationPolicy::session_context(issuer))?;

    let session_id = claims.session_id.ok_or(AuthError::MissingSessionId)?;
    let role = *claims.scope.first().ok_or(AuthError::MissingScope)?;
    let act_chain = claims
        .act
        .as_ref()
        .map(systemprompt_models::auth::ActClaim::flatten_to_chain)
        .unwrap_or_default();

    Ok(JwtUserContext {
        user_id: UserId::try_new(claims.sub).map_err(AuthError::InvalidSubject)?,
        session_id,
        role,
        user_type: claims.user_type,
        client_id: claims.client_id,
        act_chain,
        attributes: claims.attributes,
        jti: (!claims.jti.is_empty()).then(|| AccessTokenId::new(claims.jti)),
        exp: claims.exp,
    })
}
