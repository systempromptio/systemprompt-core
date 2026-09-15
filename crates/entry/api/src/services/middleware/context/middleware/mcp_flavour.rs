//! MCP request-context middleware with execution capability binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    Actor, Arc, ContextExtractor, ContextId, DynExtractor, HeaderExtractor, Next, Request,
    RequestContext, Response, SessionId, StatusCode, UserType, create_request_span,
    log_error_response, session_context_required_error,
};
use axum::response::IntoResponse;
use systemprompt_identifiers::TraceId;
use systemprompt_models::execution::context::ContextExtractionError;
use tracing::Instrument;

/// MCP flavour: headers-only extraction with session fallback.
///
/// Only a *missing* authorization header forwards the session context, so the
/// MCP proxy can return the RFC 9728 challenge that starts OAuth discovery. A
/// bearer that is present but invalid, revoked, or bound to a deleted user is
/// refused outright; it never degrades to the anonymous session.
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

    async fn execution_context(
        &self,
        headers: http::HeaderMap,
        path: String,
        token: &str,
        trace_id: TraceId,
    ) -> Result<RequestContext, Response> {
        let refuse = |message: &'static str| (StatusCode::UNAUTHORIZED, message).into_response();
        if !execution_service_allowed(&path) {
            tracing::warn!(
                %path,
                "execution capability presented outside the evaluation fixture server"
            );
            return Err((
                StatusCode::FORBIDDEN,
                "Execution capabilities reach only the evaluation fixture server",
            )
                .into_response());
        }
        let Some(capabilities) = &self.execution_capabilities else {
            return Err(refuse("Execution capabilities are unavailable"));
        };
        let Some(environment) = &self.execution_environment else {
            return Err(refuse("Execution environment is unavailable"));
        };
        let principal = match capabilities.authenticate(token, environment).await {
            Ok(principal) => principal,
            Err(error) => {
                tracing::warn!(%error, "execution capability refused");
                return Err(refuse("Invalid or expired execution capability"));
            },
        };
        let header_session = headers
            .get("x-session-id")
            .and_then(|value| value.to_str().ok());
        if header_session != Some(principal.session_id.as_str()) {
            return Err(refuse("Execution capability session mismatch"));
        }
        let context_id = ContextId::derived_from_session(&principal.session_id);
        let agent = HeaderExtractor::extract_agent_name(&headers);
        let actor = Actor::job(
            principal.identity.owner_id,
            format!("evaluation:{}", principal.identity.execution_id),
        );
        Ok(RequestContext::new(
            SessionId::new(principal.session_id.as_str()),
            trace_id,
            context_id,
            agent,
        )
        .with_actor(actor)
        .with_user_type(UserType::Mcp)
        .with_auth_token(token))
    }

    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = original_path(&request);
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
            let headers = request.headers().clone();
            let token = token.to_owned();
            let context = match self
                .execution_context(headers, path.clone(), &token, trace_id)
                .await
            {
                Ok(context) => context,
                Err(refusal) => return refusal,
            };
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
            Err(ContextExtractionError::MissingAuthHeader) => {
                if let Some(ctx) = request.extensions().get::<RequestContext>().cloned() {
                    tracing::debug!(
                        trace_id = %trace_id,
                        "MCP request without authorization header, using session context"
                    );
                    let span = create_request_span(&ctx);
                    next.run(request).instrument(span).await
                } else {
                    session_context_required_error(&trace_id, &path, &method)
                }
            },
            Err(e) => {
                let response = log_error_response(&e, &trace_id, &path, &method);
                if response.status() == StatusCode::UNAUTHORIZED {
                    invalid_token_challenge(&path)
                } else {
                    response
                }
            },
        }
    }
}

// Why: RFC 6750 §3.1 — a presented-but-invalid bearer is answered with
// `error="invalid_token"`, and RFC 9728 keeps `resource_metadata` on the
// challenge so the client can recover through OAuth discovery.
fn invalid_token_challenge(path: &str) -> Response {
    let service = path
        .strip_prefix(systemprompt_models::ApiPaths::MCP_BASE)
        .and_then(|rest| rest.trim_start_matches('/').split('/').next())
        .filter(|service| !service.is_empty())
        .unwrap_or("mcp");
    let header = format!(
        "Bearer realm=\"{service}\", \
         resource_metadata=\"/.well-known/oauth-protected-resource{path}\", \
         error=\"invalid_token\", \
         error_description=\"The access token is missing or invalid\""
    );
    let body = serde_json::json!({
        "error": "invalid_token",
        "error_description": "The access token is missing or invalid",
        "server": service,
    });
    (
        StatusCode::UNAUTHORIZED,
        [
            (http::header::CONTENT_TYPE, "application/json".to_owned()),
            (http::header::WWW_AUTHENTICATE, header),
        ],
        body.to_string(),
    )
        .into_response()
}

// Why: Axum strips the mount prefix from `req.uri()` inside a nested router.
fn original_path(request: &Request) -> String {
    request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or_else(
            || request.uri().path().to_owned(),
            |original| original.path().to_owned(),
        )
}

// Why: the sandboxed native client is handed an execution capability for the
// relay only; the token must not open the owner's whole MCP mesh.
const EXECUTION_FIXTURE_SERVICE: &str = "evaluation_fixture";

fn execution_service_allowed(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(systemprompt_models::ApiPaths::MCP_BASE) else {
        return false;
    };
    let service = rest.trim_start_matches('/').split('/').next().unwrap_or("");
    service == EXECUTION_FIXTURE_SERVICE
}
