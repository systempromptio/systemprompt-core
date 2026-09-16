//! Problem details preserve domain failures without exposing storage internals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_runtime::managed::OrchestrationError;

#[derive(Debug, thiserror::Error)]
pub(super) enum ManagedHttpError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Analytics(#[from] systemprompt_analytics::AnalyticsError),
    #[error(transparent)]
    Managed(#[from] ManagedError),
    #[error(transparent)]
    Orchestration(#[from] OrchestrationError),
    #[error("Retained operation result is unreadable: {0}")]
    Json(#[from] serde_json::Error),
}

impl IntoResponse for ManagedHttpError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Invalid(_) => StatusCode::BAD_REQUEST,
            Self::Analytics(systemprompt_analytics::AnalyticsError::InvalidArgument(_)) => {
                StatusCode::BAD_REQUEST
            },
            Self::Analytics(_) | Self::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Managed(error) | Self::Orchestration(OrchestrationError::Managed(error)) => {
                managed_status(error)
            },
            Self::Orchestration(OrchestrationError::Source(_)) => StatusCode::CONFLICT,
        };
        let title = status.canonical_reason().unwrap_or("Request failed");
        let detail = if status.is_server_error() {
            tracing::error!(error = %self, "Managed resource request failed");
            "The operation could not be completed".to_owned()
        } else {
            self.to_string()
        };
        (status, [(header::CONTENT_TYPE, "application/problem+json"), (header::CACHE_CONTROL, "no-store")],
            Json(serde_json::json!({"type":"about:blank","title":title,"status":status.as_u16(),"detail":detail}))).into_response()
    }
}

const fn managed_status(error: &ManagedError) -> StatusCode {
    match error {
        ManagedError::Unavailable => StatusCode::NOT_FOUND,
        ManagedError::Invalid(_) => StatusCode::BAD_REQUEST,
        ManagedError::Conflict(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
