use axum::http::HeaderMap;
use systemprompt_identifiers::{Actor, ContextId, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, MAX_ACT_CHAIN_DEPTH, Permission, UserType,
};
use systemprompt_models::execution::context::RequestContext;

use crate::error::{AuthError, AuthResult};
use crate::extraction::HeaderExtractor;
use crate::keys::authority;
use crate::session::ValidatedSessionClaims;

const ANONYMOUS_SESSION_ID: &str = "anonymous";
const BEARER_PREFIX: &str = "Bearer ";

/// Maximum clock-skew tolerance (seconds) for `exp`, `nbf`, and `iat`
/// validation. Pinned explicitly so deployments see this value in code
/// review rather than inheriting the `jsonwebtoken` default.
pub(super) const JWT_LEEWAY_SECONDS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Required,
    Optional,
}

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

    pub fn validate_request(
        &self,
        headers: &HeaderMap,
        mode: AuthMode,
    ) -> AuthResult<RequestContext> {
        match mode {
            AuthMode::Required => self.validate_and_fail_fast(headers),
            AuthMode::Optional => Ok(self.try_validate_or_anonymous(headers)),
        }
    }

    fn validate_and_fail_fast(&self, headers: &HeaderMap) -> AuthResult<RequestContext> {
        let token = Self::extract_token(headers).ok_or(AuthError::MissingAuthorization)?;

        let claims = self.validate_token(token)?;
        Ok(Self::create_context_from_claims(&claims, token, headers))
    }

    fn try_validate_or_anonymous(&self, headers: &HeaderMap) -> RequestContext {
        Self::extract_token(headers).map_or_else(
            || Self::create_anonymous_context(headers),
            |token| {
                self.validate_token(token)
                    .map_err(|e| {
                        tracing::debug!(error = %e, "Token validation failed, falling back to anonymous");
                        e
                    })
                    .map_or_else(
                        |_| Self::create_anonymous_context(headers),
                        |claims| Self::create_context_from_claims(&claims, token, headers),
                    )
            },
        )
    }

    fn extract_token(headers: &HeaderMap) -> Option<&str> {
        headers
            .get("authorization")
            .and_then(|h| {
                h.to_str()
                    .map_err(|e| {
                        tracing::debug!(error = %e, "Authorization header contains non-ASCII characters");
                        e
                    })
                    .ok()
            })
            .and_then(|s| s.strip_prefix(BEARER_PREFIX))
    }

    fn validate_token(&self, token: &str) -> AuthResult<ValidatedSessionClaims> {
        use jsonwebtoken::{Algorithm, Validation, decode, decode_header};

        let header = decode_header(token).map_err(AuthError::InvalidToken)?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::UnsupportedAlgorithm);
        }
        let kid = header.kid.as_deref().ok_or(AuthError::MissingKid)?;
        let key = authority::decoding_key_for_kid(kid)
            .map_err(|e| AuthError::KeyLookup(e.to_string()))?
            .ok_or_else(|| AuthError::UnknownKid(kid.to_string()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.leeway = JWT_LEEWAY_SECONDS;
        validation.validate_nbf = true;

        validation.set_issuer(&[&self.issuer]);

        let audience_strs: Vec<&str> = self.audiences.iter().map(JwtAudience::as_str).collect();
        validation.set_audience(&audience_strs);

        let token_data =
            decode::<JwtClaims>(token, key, &validation).map_err(AuthError::InvalidToken)?;

        let claims = token_data.claims;

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

    fn create_anonymous_context(headers: &HeaderMap) -> RequestContext {
        RequestContext::new(
            SessionId::new(ANONYMOUS_SESSION_ID.to_string()),
            HeaderExtractor::extract_trace_id(headers),
            HeaderExtractor::extract_context_id(headers).unwrap_or_else(ContextId::generate),
            HeaderExtractor::extract_agent_name(headers),
        )
        .with_actor(Actor::anonymous(
            systemprompt_identifiers::bootstrap::anonymous(),
        ))
        .with_user_type(UserType::Anon)
    }
}
