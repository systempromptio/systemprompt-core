use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use std::sync::Arc;

use crate::services::middleware::context::ContextExtractor;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_security::TokenExtractor;
use systemprompt_traits::AnalyticsProvider;

use super::params::{BuildContextParams, build_context, extract_common_headers};
use super::token::{JwtExtractor, JwtUserContext};
use super::validation::{validate_session_exists, validate_user_exists};

#[derive(Clone)]
pub struct JwtContextExtractor {
    jwt_extractor: Arc<JwtExtractor>,
    token_extractor: TokenExtractor,
    db_pool: DbPool,
    analytics_provider: Option<Arc<dyn AnalyticsProvider>>,
}

impl std::fmt::Debug for JwtContextExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtContextExtractor")
            .field("jwt_extractor", &self.jwt_extractor)
            .field("token_extractor", &self.token_extractor)
            .field("db_pool", &"DbPool")
            .field("analytics_provider", &self.analytics_provider.is_some())
            .finish()
    }
}

impl JwtContextExtractor {
    pub fn new(jwt_secret: &str, db_pool: &DbPool) -> Self {
        Self {
            jwt_extractor: Arc::new(JwtExtractor::new(jwt_secret)),
            token_extractor: TokenExtractor::browser_only(),
            db_pool: Arc::clone(db_pool),
            analytics_provider: None,
        }
    }

    pub fn with_analytics_provider(mut self, provider: Arc<dyn AnalyticsProvider>) -> Self {
        self.analytics_provider = Some(provider);
        self
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

        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }

        validate_user_exists(&self.db_pool, &jwt_context, "").await?;
        validate_session_exists(self.analytics_provider.as_ref(), &jwt_context, headers, "")
            .await?;

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
            .map_or_else(
                || ContextId::new(String::new()),
                |s| ContextId::new(s.to_string()),
            );

        let (trace_id, task_id, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, headers);

        Ok(build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
        }))
    }

    pub async fn extract_mcp_a2a(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
    }

    pub async fn extract_for_gateway(
        &self,
        jwt_token: &systemprompt_identifiers::JwtToken,
    ) -> Result<(RequestContext, JwtUserContext), ContextExtractionError> {
        let jwt_context = self
            .jwt_extractor
            .extract_user_context(jwt_token.as_str())
            .map_err(|e| ContextExtractionError::InvalidToken(e.to_string()))?;

        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }

        validate_user_exists(&self.db_pool, &jwt_context, "gateway").await?;

        let session_id = jwt_context.session_id.clone();
        let user_id = jwt_context.user_id.clone();
        let claims_snapshot = jwt_context.clone();

        let rc = build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id: TraceId::generate(),
            context_id: ContextId::new(String::new()),
            agent_name: AgentName::system(),
            task_id: None,
            auth_token: Some(jwt_token.as_str().to_string()),
        });
        Ok((rc, claims_snapshot))
    }

    async fn extract_from_request_impl(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        use crate::services::middleware::context::sources::{
            ContextIdSource, PayloadSource, TASK_BASED_CONTEXT_MARKER,
        };

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

        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }

        validate_user_exists(&self.db_pool, &jwt_context, " (A2A route)").await?;
        validate_session_exists(
            self.analytics_provider.as_ref(),
            &jwt_context,
            &headers,
            " (A2A route)",
        )
        .await?;

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request).await?;

        let context_source = PayloadSource::extract_context_source(&body_bytes)?;
        let (context_id, task_id_from_payload) = match context_source {
            ContextIdSource::Direct(id) => (ContextId::new(id), None),
            ContextIdSource::FromTask { task_id } => {
                (ContextId::new(TASK_BASED_CONTEXT_MARKER), Some(task_id))
            },
        };

        let (trace_id, task_id_from_header, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, &headers);

        let task_id = task_id_from_payload.or(task_id_from_header);

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
        });

        Ok((ctx, reconstructed_request))
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
