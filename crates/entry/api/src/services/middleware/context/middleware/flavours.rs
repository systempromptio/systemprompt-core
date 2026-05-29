use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_identifiers::{AgentName, ContextId};
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
        let Some(mut req_ctx) = request.extensions().get::<RequestContext>().cloned() else {
            let trace_id = HeaderExtractor::extract_trace_id(request.headers());
            let path = request.uri().path().to_owned();
            let method = request.method().to_string();
            return session_context_required_error(&trace_id, &path, &method);
        };

        let headers = request.headers();
        if let Some(context_id) = headers.get("x-context-id") {
            if let Ok(id) = context_id.to_str() {
                req_ctx.execution.context_id = ContextId::new(id.to_owned());
            }
        }

        if let Some(agent_name) = headers.get("x-agent-name") {
            if let Ok(name) = agent_name.to_str() {
                req_ctx.execution.agent_name = AgentName::new(name.to_owned());
            }
        }

        let span = create_request_span(&req_ctx);
        request.extensions_mut().insert(req_ctx);
        next.run(request).instrument(span).await
    }
}

/// Authenticated-headers flavour. Requires a real user from request headers
/// and rejects the request on extraction failure. Use this for any route
/// whose handler may not run anonymously.
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
        }
    }

    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();

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
