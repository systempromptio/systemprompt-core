//! Shared helpers for the context middlewares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::TraceId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::RequestContext;

use super::super::extractors::ContextExtractor;

pub(super) type DynExtractor = Arc<dyn ContextExtractor + Send + Sync>;

pub(super) fn create_request_span(ctx: &RequestContext) -> tracing::Span {
    tracing::info_span!(
        "request",
        user_id = %ctx.user_id(),
        session_id = %ctx.session_id(),
        trace_id = %ctx.trace_id(),
        context_id = %ctx.context_id(),
    )
}

pub(super) fn session_context_required_error(
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
