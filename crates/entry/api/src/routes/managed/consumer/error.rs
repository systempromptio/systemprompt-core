//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_marketplace::managed::ManagedError;

#[derive(Debug, Clone, Copy)]
pub(super) struct ConsumerHttpError(pub StatusCode);

impl From<ManagedError> for ConsumerHttpError {
    fn from(error: ManagedError) -> Self {
        Self(match error {
            ManagedError::Unavailable => StatusCode::FORBIDDEN,
            ManagedError::Invalid(_) => StatusCode::BAD_REQUEST,
            ManagedError::Integrity | ManagedError::Conflict(_) => StatusCode::CONFLICT,
            _ => {
                tracing::error!(%error, "Consumer evidence operation failed");
                StatusCode::INTERNAL_SERVER_ERROR
            },
        })
    }
}

impl IntoResponse for ConsumerHttpError {
    fn into_response(self) -> Response {
        super::super::contract::problem(
            self.0,
            match self.0 {
                StatusCode::UNAUTHORIZED => "Valid enrolled-device authentication is required",
                StatusCode::FORBIDDEN => "Consumer evidence is unavailable to this identity",
                StatusCode::BAD_REQUEST => "Invalid consumer evidence request",
                StatusCode::CONFLICT => {
                    "Evidence conflicts with retained installation or session records"
                },
                _ => "The consumer evidence operation could not be completed",
            },
        )
    }
}
