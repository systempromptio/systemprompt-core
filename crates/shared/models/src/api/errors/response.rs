//! HTTP status mapping and the axum response for [`ApiError`].
//!
//! The response logs once by status class with the internal cause chain as a
//! structured field; the body is the redacting wire shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;

use super::{ApiError, ErrorCode};

impl ErrorCode {
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::ValidationError => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ConflictError => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

fn cause_chain(source: Option<&(dyn Error + Send + Sync + 'static)>) -> Option<String> {
    let source = source?;
    let mut chain = source.to_string();
    let mut next = source.source();
    while let Some(cause) = next {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        next = cause.source();
    }
    Some(chain)
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.code.status_code();
        let cause = cause_chain(self.source());

        if status.is_server_error() {
            tracing::error!(
                error_code = ?self.code,
                context = %self.message,
                details = ?self.details,
                error_key = ?self.error_key,
                cause = ?cause,
                path = ?self.path,
                trace_id = ?self.trace_id,
                "API server error response"
            );
        } else if status.is_client_error() {
            tracing::warn!(
                error_code = ?self.code,
                message = %self.message,
                error_key = ?self.error_key,
                cause = ?cause,
                path = ?self.path,
                trace_id = ?self.trace_id,
                "API client error response"
            );
        }

        let mut response = (status, Json(self)).into_response();

        if status == StatusCode::UNAUTHORIZED
            && let Ok(header_value) =
                "Bearer resource_metadata=\"/.well-known/oauth-protected-resource\"".parse()
        {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, header_value);
        }

        response
    }
}
