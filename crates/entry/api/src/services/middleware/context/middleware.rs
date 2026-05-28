//! Per-flavour context middleware: typed sibling middlewares that build a
//! [`RequestContext`] for a route group, with each type encoding its own
//! caller-admission contract at the type level rather than via a runtime
//! `ContextRequirement` enum.
//!
//! Four flavours exist:
//!
//! - [`PublicContextMiddleware`] — admits `UserType::Anon`; forwards the
//!   session-derived `RequestContext` minted by `POST /oauth/session` and
//!   merges optional `x-context-id` / `x-agent-name` headers on top. Never
//!   reads or rebuilds the body.
//! - [`UserOnlyContextMiddleware`] — extracts a real user from headers; on
//!   extraction failure the request fails. Used for non-A2A authenticated
//!   routes.
//! - [`A2AContextMiddleware`] — extracts a real user AND parses the JSON-RPC
//!   body to recover `contextId` (the A2A wire spec carries it in the body,
//!   not headers). Rebuilds the body for downstream handlers.
//! - [`McpContextMiddleware`] — headers-only extraction; on extraction
//!   failure, forwards the session-derived `RequestContext` (Anon) so the
//!   downstream MCP proxy handler can answer with an RFC 9728
//!   `WWW-Authenticate` 401 challenge. The fallback is load-bearing — see
//!   `crates/tests/integration/api/routes_mcp_unauth_challenge.rs`.
//!
//! All four share the same `Arc<dyn ContextExtractor>` and the same error
//! mapping (`extraction_error_to_api_error`). Mounting a route under the
//! wrong flavour is a type error, not a runtime branch.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use systemprompt_security::HeaderExtractor;
use tracing::Instrument;

use super::extractors::ContextExtractor;
use systemprompt_identifiers::{AgentName, ContextId, TraceId};
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};

type DynExtractor = Arc<dyn ContextExtractor + Send + Sync>;

pub(crate) fn extraction_error_to_api_error(error: &ContextExtractionError) -> ApiError {
    match error {
        ContextExtractionError::MissingAuthHeader => {
            ApiError::unauthorized("Missing Authorization header")
        },
        ContextExtractionError::InvalidToken(_) => {
            ApiError::unauthorized("Invalid or expired JWT token")
        },
        ContextExtractionError::UserNotFound(_) => {
            ApiError::unauthorized("User no longer exists")
        },
        ContextExtractionError::MissingSessionId => {
            ApiError::bad_request("JWT missing required 'session_id' claim")
        },
        ContextExtractionError::MissingUserId => {
            ApiError::bad_request("JWT missing required 'sub' claim")
        },
        ContextExtractionError::MissingContextId => ApiError::bad_request(
            "Missing required 'x-context-id' header (for MCP routes) or contextId in body \
             (for A2A routes)",
        ),
        ContextExtractionError::MissingHeader(header) => {
            ApiError::bad_request(format!("Missing required header: {header}"))
        },
        ContextExtractionError::InvalidHeaderValue { header, reason } => {
            ApiError::bad_request(format!("Invalid header {header}: {reason}"))
        },
        ContextExtractionError::InvalidUserId(reason) => {
            ApiError::bad_request(format!("Invalid user_id: {reason}"))
        },
        ContextExtractionError::DatabaseError(_) => {
            ApiError::internal_error("Internal server error")
        },
        ContextExtractionError::ForbiddenHeader { header, reason } => {
            ApiError::bad_request(format!(
                "Header '{header}' is not allowed: {reason}. Use JWT authentication instead."
            ))
        },
    }
}

fn log_error_response(
    error: &ContextExtractionError,
    trace_id: &TraceId,
    path: &str,
    method: &str,
) -> Response {
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
        },
        ContextExtractionError::InvalidToken(reason) => {
            tracing::warn!(
                reason = %reason,
                error_type = "invalid_token",
                "Context extraction failed: invalid token"
            );
        },
        ContextExtractionError::UserNotFound(user_id) => {
            tracing::warn!(
                user_id = %user_id,
                error_type = "user_not_found",
                "Context extraction failed: user not found"
            );
        },
        _ => {
            tracing::warn!(
                error = %error,
                error_type = "context_extraction",
                "Context extraction failed"
            );
        },
    }

    extraction_error_to_api_error(error)
        .with_trace_id(trace_id.as_str())
        .with_path(path)
        .into_response()
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

fn session_context_required_error(
    trace_id: &TraceId,
    path: &str,
    method: &str,
) -> Response {
    tracing::error!(
        trace_id = %trace_id,
        path = %path,
        method = %method,
        "Middleware configuration error: SessionMiddleware must run before context middleware"
    );
    ApiError::internal_error("Middleware configuration error")
        .with_trace_id(trace_id.as_str())
        .with_path(path)
        .into_response()
}

/// Public route flavour. Admits `UserType::Anon` and forwards the
/// session-derived [`RequestContext`] minted by `POST /oauth/session`,
/// merging optional `x-context-id` / `x-agent-name` headers on top. Never
/// touches the request body, and never invokes the extractor — the public
/// gate has nothing to extract from anonymous traffic.
#[derive(Clone, Debug, Default)]
pub struct PublicContextMiddleware;

impl PublicContextMiddleware {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Response {
        let mut req_ctx = match request.extensions().get::<RequestContext>() {
            Some(ctx) => ctx.clone(),
            None => {
                let trace_id = HeaderExtractor::extract_trace_id(request.headers());
                let path = request.uri().path().to_owned();
                let method = request.method().to_string();
                return session_context_required_error(&trace_id, &path, &method);
            },
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

/// A2A flavour. Requires a real user and parses the JSON-RPC body to recover
/// `contextId` (the A2A wire spec carries it in the body, not headers). The
/// body is read and rebuilt so downstream handlers can deserialise it again.
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

/// MCP flavour. Extracts a real user from headers when an `Authorization`
/// header is present; otherwise forwards the session-derived
/// [`RequestContext`] (Anon) so the downstream MCP proxy handler can emit an
/// RFC 9728 `WWW-Authenticate` 401 challenge to start the OAuth dance.
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
