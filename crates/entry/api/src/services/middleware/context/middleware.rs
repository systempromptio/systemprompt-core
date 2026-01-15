use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use systemprompt_core_security::HeaderExtractor;
use tracing::Instrument;

use super::extractors::ContextExtractor;
use super::requirements::ContextRequirement;
use systemprompt_identifiers::{AgentName, ContextId, TraceId};
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};

#[derive(Debug, Clone)]
pub struct ContextMiddleware<E> {
    extractor: Arc<E>,
    auth_level: ContextRequirement,
}

impl<E> ContextMiddleware<E> {
    pub fn new(extractor: E) -> Self {
        Self {
            extractor: Arc::new(extractor),
            auth_level: ContextRequirement::default(),
        }
    }

    pub fn public(extractor: E) -> Self {
        Self {
            extractor: Arc::new(extractor),
            auth_level: ContextRequirement::None,
        }
    }

    pub fn user_only(extractor: E) -> Self {
        Self {
            extractor: Arc::new(extractor),
            auth_level: ContextRequirement::UserOnly,
        }
    }

    pub fn full(extractor: E) -> Self {
        Self {
            extractor: Arc::new(extractor),
            auth_level: ContextRequirement::UserWithContext,
        }
    }

    pub fn mcp(extractor: E) -> Self {
        Self {
            extractor: Arc::new(extractor),
            auth_level: ContextRequirement::McpWithHeaders,
        }
    }

    fn error_response(error: &ContextExtractionError) -> (StatusCode, String) {
        match error {
            ContextExtractionError::MissingAuthHeader => (
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header".to_string(),
            ),
            ContextExtractionError::InvalidToken(_) => (
                StatusCode::UNAUTHORIZED,
                "Invalid or expired JWT token".to_string(),
            ),
            ContextExtractionError::UserNotFound(_) => (
                StatusCode::UNAUTHORIZED,
                "User no longer exists".to_string(),
            ),
            ContextExtractionError::MissingSessionId => (
                StatusCode::BAD_REQUEST,
                "JWT missing required 'session_id' claim".to_string(),
            ),
            ContextExtractionError::MissingUserId => (
                StatusCode::BAD_REQUEST,
                "JWT missing required 'sub' claim".to_string(),
            ),
            ContextExtractionError::MissingContextId => (
                StatusCode::BAD_REQUEST,
                "Missing required 'x-context-id' header (for MCP routes) or contextId in body \
                 (for A2A routes)"
                    .to_string(),
            ),
            ContextExtractionError::MissingHeader(header) => (
                StatusCode::BAD_REQUEST,
                format!("Missing required header: {header}"),
            ),
            ContextExtractionError::InvalidHeaderValue { header, reason } => (
                StatusCode::BAD_REQUEST,
                format!("Invalid header {header}: {reason}"),
            ),
            ContextExtractionError::InvalidUserId(reason) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid user_id: {reason}"),
            ),
            ContextExtractionError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            ContextExtractionError::ForbiddenHeader { header, reason } => (
                StatusCode::BAD_REQUEST,
                format!(
                    "Header '{header}' is not allowed: {reason}. Use JWT authentication instead."
                ),
            ),
        }
    }

    fn log_error_response(
        error: &ContextExtractionError,
        trace_id: &TraceId,
        path: &str,
        method: &str,
    ) -> Response {
        let (status, message) = Self::error_response(error);

        let _span = tracing::error_span!(
            "context_extraction_error",
            trace_id = %trace_id,
            path = %path,
            method = %method,
        )
        .entered();

        match error {
            ContextExtractionError::DatabaseError(e) => {
                tracing::error!(
                    error = %e,
                    error_type = "database",
                    "Context extraction failed due to database error"
                );
            }
            ContextExtractionError::InvalidToken(reason) => {
                tracing::warn!(
                    reason = %reason,
                    error_type = "invalid_token",
                    "Context extraction failed: invalid token"
                );
            }
            ContextExtractionError::UserNotFound(user_id) => {
                tracing::warn!(
                    user_id = %user_id,
                    error_type = "user_not_found",
                    "Context extraction failed: user not found"
                );
            }
            _ => {
                tracing::warn!(
                    error = %error,
                    error_type = "context_extraction",
                    "Context extraction failed"
                );
            }
        }

        let mut response = (status, message).into_response();
        if let Ok(header_value) = trace_id.as_str().parse() {
            response.headers_mut().insert("x-trace-id", header_value);
        }
        response
    }
}

fn create_request_span(ctx: &RequestContext) -> tracing::Span {
    tracing::info_span!(
        "request",
        user_id = %ctx.user_id(),
        session_id = %ctx.session_id(),
        trace_id = %ctx.trace_id(),
        context_id = %ctx.context_id(),
    )
}

impl<E: ContextExtractor> ContextMiddleware<E> {
    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let requirement = request
            .extensions()
            .get::<ContextRequirement>()
            .copied()
            .unwrap_or(self.auth_level);

        if request.extensions().get::<RequestContext>().is_some()
            && self.auth_level == ContextRequirement::None
        {
            return next.run(request).await;
        }

        match requirement {
            ContextRequirement::None => self.handle_none_requirement(request, next).await,
            ContextRequirement::UserOnly => self.handle_user_only(request, next).await,
            ContextRequirement::UserWithContext => {
                self.handle_user_with_context(request, next).await
            },
            ContextRequirement::McpWithHeaders => self.handle_mcp_with_headers(request, next).await,
        }
    }

    async fn handle_none_requirement(&self, mut request: Request, next: Next) -> Response {
        let headers = request.headers();
        let mut req_ctx = if let Some(ctx) = request.extensions().get::<RequestContext>() {
            ctx.clone()
        } else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Middleware configuration error: SessionMiddleware must run before \
                 ContextMiddleware",
            )
                .into_response();
        };

        if let Some(context_id) = headers.get("x-context-id") {
            if let Ok(id) = context_id.to_str() {
                req_ctx.execution.context_id = ContextId::new(id.to_string());
            }
        }

        if let Some(agent_name) = headers.get("x-agent-name") {
            if let Ok(name) = agent_name.to_str() {
                req_ctx.execution.agent_name = AgentName::new(name.to_string());
            }
        }

        let span = create_request_span(&req_ctx);
        request.extensions_mut().insert(req_ctx);
        next.run(request).instrument(span).await
    }

    async fn handle_user_only(&self, mut request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_string();
        let method = request.method().to_string();

        match self.extractor.extract_user_only(request.headers()).await {
            Ok(context) => {
                let span = create_request_span(&context);
                request.extensions_mut().insert(context);
                next.run(request).instrument(span).await
            }
            Err(e) => Self::log_error_response(&e, &trace_id, &path, &method),
        }
    }

    async fn handle_user_with_context(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_string();
        let method = request.method().to_string();

        match self.extractor.extract_from_request(request).await {
            Ok((context, reconstructed_request)) => {
                let span = create_request_span(&context);
                let mut req = reconstructed_request;
                req.extensions_mut().insert(context);
                next.run(req).instrument(span).await
            }
            Err(e) => Self::log_error_response(&e, &trace_id, &path, &method),
        }
    }

    async fn handle_mcp_with_headers(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_string();
        let method = request.method().to_string();

        match self.extractor.extract_from_headers(request.headers()).await {
            Ok(context) => {
                let span = create_request_span(&context);
                let mut req = request;
                req.extensions_mut().insert(context);
                next.run(req).instrument(span).await
            }
            Err(e) => match request.extensions().get::<RequestContext>().cloned() {
                Some(ctx) => {
                    tracing::debug!(
                        error = %e,
                        trace_id = %trace_id,
                        "MCP header extraction failed, using session context"
                    );
                    let span = create_request_span(&ctx);
                    next.run(request).instrument(span).await
                }
                None => {
                    tracing::error!(
                        trace_id = %trace_id,
                        path = %path,
                        method = %method,
                        "Middleware configuration error: SessionMiddleware must run before ContextMiddleware"
                    );
                    let mut response = (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Middleware configuration error",
                    )
                        .into_response();
                    if let Ok(header_value) = trace_id.as_str().parse() {
                        response.headers_mut().insert("x-trace-id", header_value);
                    }
                    response
                }
            },
        }
    }
}
