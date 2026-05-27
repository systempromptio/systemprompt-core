//! Bearer-token decode for request-context middleware.
//!
//! [`extract_user_context`] validates a token's `kid`, enforces RS256, decodes
//! the [`JwtClaims`] payload, and re-derives `user_type` from `scope` so a
//! forged or mis-minted type claim cannot ride past the gate. It returns the
//! subset of claims the request-context layer consumes ([`JwtUserContext`]);
//! issuer / audience / `nbf` / leeway enforcement is the responsibility of
//! [`crate::AuthValidationService`], which is the path used for fully-trusted
//! session validation.

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use std::collections::BTreeMap;
use systemprompt_identifiers::{Actor, ClientId, SessionId, UserId};
use systemprompt_models::auth::{JwtClaims, Permission, UserType};

use crate::error::{AuthError, AuthResult};
use crate::keys::authority;

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
    let header = decode_header(token).map_err(AuthError::InvalidToken)?;
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::UnsupportedAlgorithm);
    }
    let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| AuthError::KeyLookup(e.to_string()))?
        .ok_or_else(|| AuthError::UnknownKid(kid.to_owned()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.validate_aud = false;

    let claims = decode::<JwtClaims>(token, key, &validation)
        .map_err(AuthError::InvalidToken)?
        .claims;

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
