use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::TraceId;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::context::ContextExtractionError;

pub(crate) fn extraction_error_to_api_error(error: &ContextExtractionError) -> ApiError {
    match error {
        ContextExtractionError::MissingAuthHeader => {
            ApiError::unauthorized("Missing Authorization header")
        },
        ContextExtractionError::InvalidToken(_) => {
            ApiError::unauthorized("Invalid or expired JWT token")
        },
        ContextExtractionError::Revoked => ApiError::unauthorized("Token revoked"),
        ContextExtractionError::UserNotFound(_) => ApiError::unauthorized("User no longer exists"),
        ContextExtractionError::MissingSessionId => {
            ApiError::bad_request("JWT missing required 'session_id' claim")
        },
        ContextExtractionError::MissingUserId => {
            ApiError::bad_request("JWT missing required 'sub' claim")
        },
        ContextExtractionError::MissingContextId => ApiError::bad_request(
            "Missing required 'x-context-id' header (for MCP routes) or contextId in body (for \
             A2A routes)",
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
        ContextExtractionError::ForbiddenHeader { header, reason } => ApiError::bad_request(
            format!("Header '{header}' is not allowed: {reason}. Use JWT authentication instead."),
        ),
    }
}

pub(super) fn log_error_response(
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
