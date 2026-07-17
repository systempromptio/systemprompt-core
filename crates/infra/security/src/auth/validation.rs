//! JWT validation service producing a `RequestContext` from session claims.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{Actor, ContextId, SessionId, UserId};
use systemprompt_models::auth::{JwtAudience, MAX_ACT_CHAIN_DEPTH, Permission, UserType};
use systemprompt_models::execution::context::RequestContext;

use crate::error::{AuthError, AuthResult};
use crate::extraction::{HeaderExtractor, TokenExtractor};
use crate::jwt::{ValidationPolicy, decode_rs256_claims};
use crate::session::ValidatedSessionClaims;

#[derive(Debug)]
pub struct AuthValidationService {
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl AuthValidationService {
    #[must_use]
    pub const fn new(issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self { issuer, audiences }
    }

    pub fn validate_request(&self, headers: &HeaderMap) -> AuthResult<RequestContext> {
        let token = TokenExtractor::extract_from_authorization(headers)
            .map_err(|_e| AuthError::MissingAuthorization)?;
        let claims = self.validate_token(&token)?;
        Ok(Self::create_context_from_claims(&claims, &token, headers))
    }

    fn validate_token(&self, token: &str) -> AuthResult<ValidatedSessionClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, &self.audiences);
        let claims = decode_rs256_claims(token, &policy)?;

        if let Some(ref act) = claims.act {
            let depth = act.depth();
            if depth > MAX_ACT_CHAIN_DEPTH {
                return Err(AuthError::ActChainTooDeep {
                    depth,
                    max: MAX_ACT_CHAIN_DEPTH,
                });
            }
        }

        let user_type = if claims.scope.contains(&Permission::Admin) {
            UserType::Admin
        } else {
            claims.user_type
        };

        Ok(ValidatedSessionClaims {
            user_id: UserId::new(claims.sub),
            session_id: claims
                .session_id
                .map(SessionId::new)
                .ok_or(AuthError::MissingSessionId)?,
            user_type,
            jti: claims.jti,
            exp: claims.exp,
        })
    }

    fn create_context_from_claims(
        claims: &ValidatedSessionClaims,
        token: &str,
        headers: &HeaderMap,
    ) -> RequestContext {
        let session_id = claims.session_id.clone();
        let user_id = claims.user_id.clone();

        RequestContext::new(
            session_id,
            HeaderExtractor::extract_trace_id(headers),
            HeaderExtractor::extract_context_id(headers).unwrap_or_else(ContextId::generate),
            HeaderExtractor::extract_agent_name(headers),
        )
        .with_actor(Actor::user(user_id))
        .with_auth_token(token)
        .with_user_type(claims.user_type)
        .with_jti(claims.jti.clone())
        .with_token_exp(claims.exp)
    }
}
