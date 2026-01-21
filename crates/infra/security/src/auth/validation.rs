use anyhow::{anyhow, Result};
use axum::http::HeaderMap;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::auth::{JwtAudience, JwtClaims, Permission, UserType};
use systemprompt_models::execution::context::RequestContext;

use crate::extraction::HeaderExtractor;
use crate::session::ValidatedSessionClaims;

const ANONYMOUS_SESSION_ID: &str = "anonymous";
const TEST_SESSION_ID: &str = "test";
const TEST_TRACE_ID: &str = "test-trace";
const TEST_CONTEXT_ID: &str = "test-context";
const TEST_AGENT_NAME: &str = "test-agent";
const TEST_USER_ID: &str = "test-user";
const BEARER_PREFIX: &str = "Bearer ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Required,
    Optional,
    Disabled,
}

#[derive(Debug)]
pub struct AuthValidationService {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl AuthValidationService {
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    pub fn validate_request(&self, headers: &HeaderMap, mode: AuthMode) -> Result<RequestContext> {
        match mode {
            AuthMode::Required => self.validate_and_fail_fast(headers),
            AuthMode::Optional => Ok(self.try_validate_or_anonymous(headers)),
            AuthMode::Disabled => Ok(Self::create_test_context()),
        }
    }

    fn validate_and_fail_fast(&self, headers: &HeaderMap) -> Result<RequestContext> {
        let token =
            Self::extract_token(headers).ok_or_else(|| anyhow!("Missing authorization header"))?;

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

    fn validate_token(&self, token: &str) -> Result<ValidatedSessionClaims> {
        use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

        let mut validation = Validation::new(Algorithm::HS256);

        validation.set_issuer(&[&self.issuer]);

        let audience_strs: Vec<&str> = self.audiences.iter().map(JwtAudience::as_str).collect();
        validation.set_audience(&audience_strs);

        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| anyhow!("Invalid JWT token: {e}"))?;

        let claims = token_data.claims;

        let user_type = if claims.scope.contains(&Permission::Admin) {
            UserType::Admin
        } else {
            claims.user_type
        };

        Ok(ValidatedSessionClaims {
            user_id: claims.sub,
            session_id: claims
                .session_id
                .ok_or_else(|| anyhow!("Missing session_id in token"))?,
            user_type,
        })
    }

    fn create_context_from_claims(
        claims: &ValidatedSessionClaims,
        token: &str,
        headers: &HeaderMap,
    ) -> RequestContext {
        let session_id = SessionId::new(claims.session_id.clone());
        let user_id = UserId::new(claims.user_id.clone());

        RequestContext::new(
            session_id,
            HeaderExtractor::extract_trace_id(headers),
            HeaderExtractor::extract_context_id(headers),
            HeaderExtractor::extract_agent_name(headers),
        )
        .with_user_id(user_id)
        .with_auth_token(token)
        .with_user_type(claims.user_type)
    }

    fn create_anonymous_context(headers: &HeaderMap) -> RequestContext {
        RequestContext::new(
            SessionId::new(ANONYMOUS_SESSION_ID.to_string()),
            HeaderExtractor::extract_trace_id(headers),
            HeaderExtractor::extract_context_id(headers),
            HeaderExtractor::extract_agent_name(headers),
        )
        .with_user_id(UserId::anonymous())
        .with_user_type(UserType::Anon)
    }

    fn create_test_context() -> RequestContext {
        RequestContext::new(
            SessionId::new(TEST_SESSION_ID.to_string()),
            TraceId::new(TEST_TRACE_ID.to_string()),
            ContextId::new(TEST_CONTEXT_ID.to_string()),
            AgentName::new(TEST_AGENT_NAME.to_string()),
        )
        .with_user_id(UserId::new(TEST_USER_ID.to_string()))
        .with_user_type(UserType::User)
    }
}
