//! Problem details preserve domain failures without exposing storage internals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_evaluation::EvaluationError;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::managed::RevisionBundleError;
use systemprompt_runtime::optimization::OptimizationError;

#[derive(Debug, thiserror::Error)]
pub(super) enum OptimizationHttpError {
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Analytics(#[from] systemprompt_analytics::AnalyticsError),
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    #[error(transparent)]
    Managed(#[from] ManagedError),
    #[error(transparent)]
    Optimization(#[from] OptimizationError),
}

impl IntoResponse for OptimizationHttpError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Analytics(systemprompt_analytics::AnalyticsError::InvalidArgument(_)) => {
                StatusCode::BAD_REQUEST
            },
            Self::Analytics(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Evaluation(error) | Self::Optimization(OptimizationError::Evaluation(error)) => {
                evaluation_status(error)
            },
            Self::Managed(error) | Self::Optimization(OptimizationError::Managed(error)) => {
                managed_status(error)
            },
            Self::Optimization(OptimizationError::Bundle(error)) => bundle_status(error),
            Self::Optimization(OptimizationError::Source(_)) => StatusCode::CONFLICT,
            Self::Optimization(OptimizationError::Json(_)) => StatusCode::BAD_REQUEST,
        };
        let title = status.canonical_reason().unwrap_or("Request failed");
        let detail = if status.is_server_error() {
            tracing::error!(error = %self, "Optimization request failed");
            "The operation could not be completed".to_owned()
        } else {
            self.to_string()
        };
        (status, [(header::CONTENT_TYPE, "application/problem+json"), (header::CACHE_CONTROL, "no-store")],
            Json(serde_json::json!({"type":"about:blank","title":title,"status":status.as_u16(),"detail":detail}))).into_response()
    }
}

const fn evaluation_status(error: &EvaluationError) -> StatusCode {
    match error {
        EvaluationError::ResourceNotFound(_) => StatusCode::NOT_FOUND,
        EvaluationError::InvalidSpec(_) => StatusCode::BAD_REQUEST,
        EvaluationError::ExperimentConflict(_) | EvaluationError::BudgetExhausted { .. } => {
            StatusCode::CONFLICT
        },
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

const fn bundle_status(error: &RevisionBundleError) -> StatusCode {
    match error {
        RevisionBundleError::MissingRevision(_) => StatusCode::NOT_FOUND,
        RevisionBundleError::Invalid(_) => StatusCode::BAD_REQUEST,
        RevisionBundleError::Integrity | RevisionBundleError::Json(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        },
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
