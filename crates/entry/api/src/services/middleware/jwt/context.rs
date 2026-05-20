use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use std::sync::Arc;

use crate::services::middleware::context::ContextExtractor;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_security::TokenExtractor;
use systemprompt_traits::{AnalyticsProvider, AuthUser, UserProvider};

use super::params::{BuildContextParams, build_context, extract_common_headers};
use super::token::{JwtExtractor, JwtUserContext};
use super::validation::{UserCache, user_is_admin, validate_session_exists, validate_user_exists};

#[derive(Clone)]
pub struct JwtContextExtractor {
    jwt_extractor: Arc<JwtExtractor>,
    token_extractor: TokenExtractor,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    user_provider: Arc<dyn UserProvider>,
    user_cache: Arc<UserCache>,
}

impl std::fmt::Debug for JwtContextExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtContextExtractor")
            .field("jwt_extractor", &self.jwt_extractor)
            .field("token_extractor", &self.token_extractor)
            .finish_non_exhaustive()
    }
}

impl JwtContextExtractor {
    pub fn new(
        jwt_secret: &str,
        analytics_provider: Arc<dyn AnalyticsProvider>,
        user_provider: Arc<dyn UserProvider>,
    ) -> Self {
        Self {
            jwt_extractor: Arc::new(JwtExtractor::new(jwt_secret)),
            token_extractor: TokenExtractor::browser_only(),
            analytics_provider,
            user_provider,
            user_cache: UserCache::new(),
        }
    }

    fn extract_jwt_context(
        &self,
        headers: &HeaderMap,
    ) -> Result<JwtUserContext, ContextExtractionError> {
        let token = self
            .token_extractor
            .extract(headers)
            .map_err(|_| ContextExtractionError::MissingAuthHeader)?;
        self.jwt_extractor
            .extract_user_context(&token)
            .map_err(|e| ContextExtractionError::InvalidToken(e.to_string()))
    }

    async fn validate(
        &self,
        jwt_context: &JwtUserContext,
        route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError> {
        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }
        let validated = validate_user_exists(
            &self.user_provider,
            &self.user_cache,
            jwt_context,
            route_context,
        )
        .await?;
        validate_session_exists(&self.analytics_provider, jwt_context, route_context).await?;
        Ok(validated.user)
    }

    pub async fn extract_standard(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        let has_auth = headers.get("authorization").is_some();
        let has_context_headers =
            headers.get("x-user-id").is_some() && headers.get("x-session-id").is_some();

        if has_context_headers && !has_auth {
            return Err(ContextExtractionError::ForbiddenHeader {
                header: "X-User-ID/X-Session-ID".to_string(),
                reason: "Context headers require valid JWT for authentication".to_string(),
            });
        }

        let jwt_context = self.extract_jwt_context(headers)?;
        let user = self.validate(&jwt_context, "").await?;

        let session_id = headers
            .get("x-session-id")
            .and_then(|h| h.to_str().ok())
            .map_or_else(
                || jwt_context.session_id.clone(),
                |s| SessionId::new(s.to_string()),
            );

        let user_id = headers
            .get("x-user-id")
            .and_then(|h| h.to_str().ok())
            .map_or_else(
                || jwt_context.user_id.clone(),
                |s| UserId::new(s.to_string()),
            );

        let context_id = headers
            .get("x-context-id")
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty())
            .and_then(|s| ContextId::try_new(s).ok())
            .unwrap_or_else(ContextId::generate);

        let (trace_id, task_id, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, headers);

        let user_type = resolve_user_type(jwt_context.user_type, &user);

        Ok(build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
            user_type,
        }))
    }

    pub async fn extract_mcp_a2a(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
    }

    pub async fn decode_for_gateway(
        &self,
        jwt_token: &systemprompt_identifiers::JwtToken,
    ) -> Result<JwtUserContext, ContextExtractionError> {
        let jwt_context = self
            .jwt_extractor
            .extract_user_context(jwt_token.as_str())
            .map_err(|e| ContextExtractionError::InvalidToken(e.to_string()))?;

        let _ = self.validate(&jwt_context, "gateway").await?;
        Ok(jwt_context)
    }

    async fn extract_from_request_impl(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        use crate::services::middleware::context::sources::{ContextIdSource, PayloadSource};

        let headers = request.headers().clone();
        let has_auth = headers.get("authorization").is_some();

        if headers.get("x-context-id").is_some() && !has_auth {
            return Err(ContextExtractionError::ForbiddenHeader {
                header: "X-Context-ID".to_string(),
                reason: "Context ID must be in request body (A2A spec). Use contextId field in \
                         message."
                    .to_string(),
            });
        }

        let jwt_context = self.extract_jwt_context(&headers)?;
        let user = self.validate(&jwt_context, " (A2A route)").await?;

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request).await?;

        let context_source = PayloadSource::extract_context_source(&body_bytes)?;
        let (context_id, task_id_from_payload) = match context_source {
            ContextIdSource::Direct(id) => (
                ContextId::try_new(id).map_err(|e| ContextExtractionError::InvalidHeaderValue {
                    header: "contextId".to_string(),
                    reason: e.to_string(),
                })?,
                None,
            ),
            ContextIdSource::FromTask { task_id } => (ContextId::generate(), Some(task_id)),
        };

        let (trace_id, task_id_from_header, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, &headers);

        let task_id = task_id_from_payload.or(task_id_from_header);
        let user_type = resolve_user_type(jwt_context.user_type, &user);

        let session_id = jwt_context.session_id.clone();
        let user_id = jwt_context.user_id.clone();
        let ctx = build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
            user_type,
        });

        Ok((ctx, reconstructed_request))
    }
}

/// Settle the JWT-claimed `user_type` against the authoritative `users` row.
///
/// Human types (`Admin`, `User`) are downgraded if the database says the user
/// is no longer an admin; an `Admin` JWT for a non-admin row is rewritten to
/// `User`. Machine types (`Service`, `A2a`, `Mcp`, `Anon`) are trusted from
/// the JWT — they are minted by the OAuth layer and not reflected in the
/// `users.roles` column.
fn resolve_user_type(claimed: UserType, user: &AuthUser) -> UserType {
    match claimed {
        UserType::Admin if !user_is_admin(user) => UserType::User,
        other => other,
    }
}

#[async_trait]
impl ContextExtractor for JwtContextExtractor {
    async fn extract_from_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
    }

    async fn extract_from_request(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        self.extract_from_request_impl(request).await
    }

    async fn extract_user_only(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
    }
}
