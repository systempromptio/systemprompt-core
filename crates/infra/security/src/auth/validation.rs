use axum::http::HeaderMap;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::auth::{JwtAudience, JwtClaims, Permission, UserType};
use systemprompt_models::execution::context::RequestContext;

use crate::error::{AuthError, AuthResult};
use crate::extraction::HeaderExtractor;
use crate::session::ValidatedSessionClaims;

const ANONYMOUS_SESSION_ID: &str = "anonymous";
const TEST_SESSION_ID: &str = "test";
const TEST_TRACE_ID: &str = "test-trace";
const TEST_CONTEXT_ID: &str = "test-context";
const TEST_AGENT_NAME: &str = "test-agent";
const TEST_USER_ID: &str = "test-user";
const BEARER_PREFIX: &str = "Bearer ";

/// Authentication mode applied to a single inbound request.
///
/// Selected per route by the HTTP layer; controls whether a missing or
/// invalid token is fatal, falls back to an anonymous identity, or is
/// bypassed entirely for development/test profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Reject the request unless a valid bearer token is present.
    Required,
    /// Use the bearer token if valid, otherwise produce an anonymous
    /// `RequestContext`.
    Optional,
    /// Skip validation and produce a deterministic test `RequestContext`.
    Disabled,
}

/// Validates inbound request authentication material and produces a
/// [`RequestContext`].
///
/// The service is cheap to clone (`String` + `Vec`) and is typically held
/// behind an `Arc` inside the API entry-point state.
#[derive(Debug)]
pub struct AuthValidationService {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl AuthValidationService {
    /// Constructs a new validation service.
    ///
    /// `secret` is the HMAC-SHA256 signing secret, `issuer` is the
    /// expected `iss` claim, and `audiences` is the allowlist of
    /// acceptable `aud` claims.
    #[must_use]
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    /// Validates `headers` according to `mode` and produces a
    /// [`RequestContext`].
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] when `mode` is [`AuthMode::Required`] and the
    /// bearer token is missing, malformed, or fails JWT validation.
    pub fn validate_request(
        &self,
        headers: &HeaderMap,
        mode: AuthMode,
    ) -> AuthResult<RequestContext> {
        match mode {
            AuthMode::Required => self.validate_and_fail_fast(headers),
            AuthMode::Optional => Ok(self.try_validate_or_anonymous(headers)),
            AuthMode::Disabled => Ok(Self::create_test_context()),
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
        use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

        let mut validation = Validation::new(Algorithm::HS256);

        validation.set_issuer(&[&self.issuer]);

        let audience_strs: Vec<&str> = self.audiences.iter().map(JwtAudience::as_str).collect();
        validation.set_audience(&audience_strs);

        let token_data = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(AuthError::InvalidToken)?;

        let claims = token_data.claims;

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
