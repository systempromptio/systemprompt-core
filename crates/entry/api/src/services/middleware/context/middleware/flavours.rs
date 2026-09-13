//! Route-specific context middleware flavours.
//!
//! Each flavour decides how a `RequestContext` is established for a class of
//! route: [`PublicContextMiddleware`] admits anonymous traffic,
//! [`UserOnlyContextMiddleware`] requires a real user from headers,
//! [`A2AContextMiddleware`] recovers the context id from the JSON-RPC body, and
//! [`McpContextMiddleware`] falls back to the session context so the MCP proxy
//! can issue an OAuth challenge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_security::HeaderExtractor;
use tracing::Instrument;

use super::super::extractors::ContextExtractor;
use super::error::log_error_response;
use super::support::{DynExtractor, create_request_span, session_context_required_error};

/// Public route flavour: admits `UserType::Anon`.
///
/// Forwards the session-derived [`RequestContext`] minted by
/// `POST /oauth/session`, merging optional `x-context-id` / `x-agent-name`
/// headers on top. Never touches the request body, and never invokes the
/// extractor — the public gate has nothing to extract from anonymous traffic.
#[derive(Clone, Copy, Debug, Default)]
pub struct PublicContextMiddleware;

impl PublicContextMiddleware {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Response {
        let Some(req_ctx) = Self::resolve(&request) else {
            let trace_id = HeaderExtractor::extract_trace_id(request.headers());
            let path = request.uri().path().to_owned();
            let method = request.method().to_string();
            return session_context_required_error(&trace_id, &path, &method);
        };

        let span = create_request_span(&req_ctx);
        request.extensions_mut().insert(req_ctx);
        next.run(request).instrument(span).await
    }

    pub async fn seed(&self, mut request: Request, next: Next) -> Response {
        let Some(req_ctx) = Self::resolve(&request) else {
            let trace_id = HeaderExtractor::extract_trace_id(request.headers());
            let path = request.uri().path().to_owned();
            let method = request.method().to_string();
            return session_context_required_error(&trace_id, &path, &method);
        };

        request.extensions_mut().insert(req_ctx);
        next.run(request).await
    }

    fn resolve(request: &Request) -> Option<RequestContext> {
        let mut req_ctx = request.extensions().get::<RequestContext>().cloned()?;

        let headers = request.headers();
        if let Some(context_id) = headers.get("x-context-id")
            && let Ok(id) = context_id.to_str()
        {
            match ContextId::try_new(id.to_owned()) {
                Ok(parsed) => req_ctx.execution.context_id = parsed,
                Err(e) => {
                    tracing::warn!(error = %e, "ignoring malformed x-context-id header");
                },
            }
        }

        if let Some(agent_name) = headers.get("x-agent-name")
            && let Ok(name) = agent_name.to_str()
        {
            match AgentName::try_new(name.to_owned()) {
                Ok(parsed) => req_ctx.execution.agent_name = parsed,
                Err(e) => {
                    tracing::warn!(error = %e, "ignoring malformed x-agent-name header");
                },
            }
        }

        Some(req_ctx)
    }
}

#[derive(Clone)]
pub struct UserOnlyContextMiddleware {
    extractor: DynExtractor,
}

impl std::fmt::Debug for UserOnlyContextMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserOnlyContextMiddleware").finish()
    }
}

impl UserOnlyContextMiddleware {
    pub fn new<E>(extractor: E) -> Self
    where
        E: ContextExtractor + Send + Sync + 'static,
    {
        Self {
            extractor: Arc::new(extractor),
        }
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();

        match self.extractor.extract_from_headers(request.headers()).await {
            Ok(context) => {
                let span = create_request_span(&context);
                request.extensions_mut().insert(context);
                next.run(request).instrument(span).await
            },
            Err(e) => log_error_response(&e, &trace_id, &path, &method),
        }
    }
}

/// A2A flavour: requires a real user.
///
/// Parses the JSON-RPC body to recover `contextId` (the A2A wire spec carries
/// it in the body, not headers). The body is read and rebuilt so downstream
/// handlers can deserialise it again.
#[derive(Clone)]
pub struct A2AContextMiddleware {
    extractor: DynExtractor,
}

impl std::fmt::Debug for A2AContextMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2AContextMiddleware").finish()
    }
}

impl A2AContextMiddleware {
    pub fn new<E>(extractor: E) -> Self
    where
        E: ContextExtractor + Send + Sync + 'static,
    {
        Self {
            extractor: Arc::new(extractor),
        }
    }

    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();

        match self.extractor.extract_from_request(request).await {
            Ok((context, reconstructed_request)) => {
                let span = create_request_span(&context);
                let mut req = reconstructed_request;
                req.extensions_mut().insert(context);
                next.run(req).instrument(span).await
            },
            Err(e) => log_error_response(&e, &trace_id, &path, &method),
        }
    }
}

/// MCP flavour: headers-only extraction with session fallback.
///
/// Extracts a real user from headers when an `Authorization` header is present;
/// otherwise forwards the session-derived [`RequestContext`] (Anon) so the
/// downstream MCP proxy handler can emit an RFC 9728 `WWW-Authenticate` 401
/// challenge to start the OAuth dance.
///
/// The session-context fallback is load-bearing: MCP clients (Cowork,
/// Claude Code, etc.) only begin OAuth discovery on a 401 carrying the
/// challenge — collapsing this to a 4xx-without-challenge breaks them. See
/// `crates/tests/integration/api/routes_mcp_unauth_challenge.rs`.
#[derive(Clone)]
pub struct McpContextMiddleware {
    extractor: DynExtractor,
    execution_capabilities: Option<systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository>,
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

        let execution_token = request.headers().get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| value.starts_with(systemprompt_evaluation::repository::experiments::EXECUTION_TOKEN_PREFIX));
        if let Some(token) = execution_token {
            let Some(capabilities) = &self.execution_capabilities else {
                return (StatusCode::UNAUTHORIZED, "Execution capabilities are unavailable").into_response();
            };
            let Some(environment) = &self.execution_environment else {
                return (StatusCode::UNAUTHORIZED, "Execution environment is unavailable").into_response();
            };
            let principal = match capabilities.authenticate(token, environment).await {
                Ok(principal) => principal,
                Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid or expired execution capability").into_response(),
            };
            let header_session = request.headers().get("x-session-id").and_then(|value| value.to_str().ok());
            if header_session != Some(principal.session_id.as_str()) {
                return (StatusCode::UNAUTHORIZED, "Execution capability session mismatch").into_response();
            }
            let context_id = ContextId::derived_from_session(&principal.session_id);
            let agent = HeaderExtractor::extract_agent_name(request.headers());
            let actor = Actor::job(principal.identity.owner_id, format!("evaluation:{}", principal.identity.execution_id));
            let context = RequestContext::new(SessionId::new(principal.session_id.as_str()), trace_id, context_id, agent)
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
