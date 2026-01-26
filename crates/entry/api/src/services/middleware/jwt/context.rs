use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use std::sync::Arc;

use crate::services::middleware::context::ContextExtractor;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AgentName, ContextId, SessionId, SessionSource, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_security::{HeaderExtractor, TokenExtractor};
use systemprompt_traits::{AnalyticsProvider, CreateSessionInput};
use systemprompt_users::UserService;

use super::token::{JwtExtractor, JwtUserContext};

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
            db_pool: db_pool.clone(),
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

    async fn validate_user_exists(
        &self,
        jwt_context: &JwtUserContext,
        route_context: &str,
    ) -> Result<(), ContextExtractionError> {
        let user_service = UserService::new(&self.db_pool).map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to create user service: {e}"))
        })?;
        let user_exists = user_service
            .find_by_id(&jwt_context.user_id)
            .await
            .map_err(|e| {
                ContextExtractionError::DatabaseError(format!(
                    "Failed to check user existence: {e}"
                ))
            })?;

        if user_exists.is_none() {
            tracing::info!(
                session_id = %jwt_context.session_id.as_str(),
                user_id = %jwt_context.user_id.as_str(),
                route = %route_context,
                "JWT validation failed: User no longer exists in database"
            );

            return Err(ContextExtractionError::UserNotFound(format!(
                "User {} no longer exists",
                jwt_context.user_id.as_str()
            )));
        }
        Ok(())
    }

    async fn validate_session_exists(
        &self,
        jwt_context: &JwtUserContext,
        headers: &HeaderMap,
        route_context: &str,
    ) -> Result<(), ContextExtractionError> {
        let Some(analytics_provider) = &self.analytics_provider else {
            return Ok(());
        };

        let session_exists = analytics_provider
            .find_session_by_id(&jwt_context.session_id)
            .await
            .map_err(|e| {
                ContextExtractionError::DatabaseError(format!("Failed to check session: {e}"))
            })?
            .is_some();

        if session_exists {
            return Ok(());
        }

        tracing::info!(
            session_id = %jwt_context.session_id.as_str(),
            user_id = %jwt_context.user_id.as_str(),
            route = %route_context,
            "Creating missing session for legacy token"
        );

        let config = systemprompt_models::Config::get().map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to get config: {e}"))
        })?;
        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(config.jwt_access_token_expiration);
        let analytics = analytics_provider.extract_analytics(headers, None);
        let session_source = jwt_context
            .client_id
            .as_ref()
            .map(|c| SessionSource::from_client_id(c.as_str()))
            .unwrap_or(SessionSource::Api);

        analytics_provider
            .create_session(CreateSessionInput {
                session_id: &jwt_context.session_id,
                user_id: Some(&jwt_context.user_id),
                analytics: &analytics,
                session_source,
                is_bot: false,
                expires_at,
            })
            .await
            .map_err(|e| {
                ContextExtractionError::DatabaseError(format!("Failed to create session: {e}"))
            })?;

        Ok(())
    }

    fn extract_common_headers(
        &self,
        headers: &HeaderMap,
    ) -> (TraceId, Option<TaskId>, Option<String>, AgentName) {
        (
            HeaderExtractor::extract_trace_id(headers),
            HeaderExtractor::extract_task_id(headers),
            self.token_extractor.extract(headers).ok(),
            HeaderExtractor::extract_agent_name(headers),
        )
    }

    fn build_context(
        jwt_context: &JwtUserContext,
        session_id: SessionId,
        user_id: UserId,
        trace_id: TraceId,
        context_id: ContextId,
        agent_name: AgentName,
        task_id: Option<TaskId>,
        auth_token: Option<String>,
    ) -> RequestContext {
        let mut ctx = RequestContext::new(session_id, trace_id, context_id, agent_name)
            .with_user_id(user_id)
            .with_user_type(jwt_context.user_type);

        if let Some(client_id) = jwt_context.client_id.clone() {
            ctx = ctx.with_client_id(client_id);
        }
        if let Some(t_id) = task_id {
            ctx = ctx.with_task_id(t_id);
        }
        if let Some(token) = auth_token {
            ctx = ctx.with_auth_token(token);
        }
        ctx
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

        self.validate_user_exists(&jwt_context, "").await?;
        self.validate_session_exists(&jwt_context, headers, "")
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

        let (trace_id, task_id, auth_token, agent_name) = self.extract_common_headers(headers);

        Ok(Self::build_context(
            &jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
        ))
    }

    pub async fn extract_mcp_a2a(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
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

        self.validate_user_exists(&jwt_context, " (A2A route)")
            .await?;
        self.validate_session_exists(&jwt_context, &headers, " (A2A route)")
            .await?;

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request).await?;

        let context_source = PayloadSource::extract_context_source(&body_bytes)?;
        let (context_id, task_id_from_payload) = match context_source {
            ContextIdSource::Direct(id) => (ContextId::new(id), None),
            ContextIdSource::FromTask { task_id } => (
                ContextId::new(TASK_BASED_CONTEXT_MARKER),
                Some(TaskId::new(task_id)),
            ),
        };

        let (trace_id, task_id_from_header, auth_token, agent_name) =
            self.extract_common_headers(&headers);

        let task_id = task_id_from_payload.or(task_id_from_header);

        let ctx = Self::build_context(
            &jwt_context,
            jwt_context.session_id.clone(),
            jwt_context.user_id.clone(),
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
        );

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
