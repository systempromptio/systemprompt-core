//! MCP request-context middleware with execution capability binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

/// MCP flavour: headers-only extraction with session fallback.
///
/// A missing authorization header forwards the session context so the MCP
/// proxy can return the RFC 9728 challenge that starts OAuth discovery.
#[derive(Clone)]
pub struct McpContextMiddleware {
    extractor: DynExtractor,
    execution_capabilities:
        Option<systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository>,
    execution_environment: Option<String>,
}

impl std::fmt::Debug for McpContextMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpContextMiddleware").finish()
    }
}

impl McpContextMiddleware {
    pub fn new<E>(extractor: E) -> Self
    where
        E: ContextExtractor + Send + Sync + 'static,
    {
        Self {
            extractor: Arc::new(extractor),
            execution_capabilities: None,
            execution_environment: None,
        }
    }

    pub fn with_execution_capabilities(
        mut self,
        capabilities: systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository,
        environment: String,
    ) -> Self {
        self.execution_capabilities = Some(capabilities);
        self.execution_environment = Some(environment);
        self
    }

    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();

        let execution_token = request
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| {
                value.starts_with(
                    systemprompt_evaluation::repository::experiments::EXECUTION_TOKEN_PREFIX,
                )
            });
        if let Some(token) = execution_token {
            let Some(capabilities) = &self.execution_capabilities else {
                return (
                    StatusCode::UNAUTHORIZED,
                    "Execution capabilities are unavailable",
                )
                    .into_response();
            };
            let Some(environment) = &self.execution_environment else {
                return (
                    StatusCode::UNAUTHORIZED,
                    "Execution environment is unavailable",
                )
                    .into_response();
            };
            let principal = match capabilities.authenticate(token, environment).await {
                Ok(principal) => principal,
                Err(_) => {
                    return (
                        StatusCode::UNAUTHORIZED,
                        "Invalid or expired execution capability",
                    )
                        .into_response();
                },
            };
            let header_session = request
                .headers()
                .get("x-session-id")
                .and_then(|value| value.to_str().ok());
            if header_session != Some(principal.session_id.as_str()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    "Execution capability session mismatch",
                )
                    .into_response();
            }
            let context_id = ContextId::derived_from_session(&principal.session_id);
            let agent = HeaderExtractor::extract_agent_name(request.headers());
            let actor = Actor::job(
                principal.identity.owner_id,
                format!("evaluation:{}", principal.identity.execution_id),
            );
            let context = RequestContext::new(
                SessionId::new(principal.session_id.as_str()),
                trace_id,
                context_id,
                agent,
            )
            .with_actor(actor)
            .with_user_type(UserType::Mcp)
            .with_auth_token(token);
            let span = create_request_span(&context);
            let mut req = request;
            req.extensions_mut().insert(context);
            return next.run(req).instrument(span).await;
        }

        match self.extractor.extract_from_headers(request.headers()).await {
            Ok(context) => {
                let span = create_request_span(&context);
                let mut req = request;
                req.extensions_mut().insert(context);
                next.run(req).instrument(span).await
            },
            Err(e) => {
                if let Some(ctx) = request.extensions().get::<RequestContext>().cloned() {
                    tracing::debug!(
                        error = %e,
                        trace_id = %trace_id,
                        "MCP header extraction failed, using session context"
                    );
                    let span = create_request_span(&ctx);
                    next.run(request).instrument(span).await
                } else {
                    session_context_required_error(&trace_id, &path, &method)
                }
            },
        }
    }
}
