//! Evaluator worker transport with server-owned identity and fenced mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_evaluation::EvaluationError;

#[derive(Debug, thiserror::Error)]
pub(super) enum WorkerHttpError {
    #[error("Worker authentication required")]
    Unauthorized,
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
}

impl IntoResponse for WorkerHttpError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Evaluation(EvaluationError::ResourceNotFound(_)) => StatusCode::NOT_FOUND,
            Self::Evaluation(EvaluationError::InvalidSpec(_)) => StatusCode::BAD_REQUEST,
            Self::Evaluation(EvaluationError::ExperimentConflict(_)) => StatusCode::CONFLICT,
            Self::Evaluation(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status.is_server_error() {
            tracing::error!(error = %self, "Evaluation worker request failed");
        } else {
            tracing::warn!(error = %self, "Evaluation worker request rejected");
        }
        (
            status,
            status.canonical_reason().unwrap_or("Request failed"),
        )
            .into_response()
    }
}
