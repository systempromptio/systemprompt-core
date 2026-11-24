//! The HTTP response a proxy error renders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ProxyError;
use crate::error::ApiHttpError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;

impl From<ProxyError> for StatusCode {
    fn from(error: ProxyError) -> Self {
        error.to_status_code()
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        if let Self::AuthChallenge(challenge) = self {
            return challenge.into_response();
        }
        let status = self.to_status_code();
        let error_key = self.error_key();
        let api_error = match status {
            StatusCode::NOT_FOUND => ApiError::not_found(self.to_string()),
            StatusCode::UNAUTHORIZED => ApiError::unauthorized(self.to_string()),
            StatusCode::FORBIDDEN => ApiError::forbidden(self.to_string()),
            StatusCode::BAD_REQUEST => ApiError::bad_request(self.to_string()),
            StatusCode::CONFLICT => ApiError::conflict(self.to_string()),
            StatusCode::PAYLOAD_TOO_LARGE => ApiError::payload_too_large(self.to_string()),
            StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::BAD_GATEWAY
            | StatusCode::GATEWAY_TIMEOUT => ApiError::service_unavailable("Proxy request failed")
                .with_details(cause_chain(&self)),
            _ => ApiError::internal("Proxy request failed", self.to_string())
                .with_details(cause_chain(&self)),
        };
        ApiHttpError::from(api_error.with_error_key(error_key)).into_response()
    }
}

// Why: ProxyError owns an axum response (AuthChallenge), which is not Sync, so
// it cannot ride as an ApiError source; the chain goes to the 5xx log as
// details.
pub(super) fn cause_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        next = cause.source();
    }
    chain
}
