//! MCP request-context middleware: headers-only extraction with session fallback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    Arc, ContextExtractor, DynExtractor, HeaderExtractor, Next, Request, RequestContext, Response,
    StatusCode, create_request_span, log_error_response, session_context_required_error,
};
use axum::response::IntoResponse;
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
        let path = original_path(&request);
        let method = request.method().to_string();

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
