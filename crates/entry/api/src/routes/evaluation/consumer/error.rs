//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::{StatusCode, header};
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
        (
            self.0,
            [
                (header::CONTENT_TYPE, "application/problem+json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Json(serde_json::json!({
                "type": "about:blank", "status": self.0.as_u16(),
                "title": self.0.canonical_reason().unwrap_or("Request failed")
            })),
        )
            .into_response()
    }
}
