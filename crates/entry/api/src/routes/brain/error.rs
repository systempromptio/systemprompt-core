//! [`BrainHttpError`]: the one mapping of brain failures onto the HTTP
//! error envelope. An unknown fact is 404, a refused action 403, an
//! erased identity 409, an undeclared kind or an invalid subject, statement
//! or filter 422, each with its `error_key`. A storage, outbox or
//! notification failure is a fixed 503 body and any other failure the fixed
//! 500 body, with the cause logged once by [`ApiHttpError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::{IntoResponse, Response};
use systemprompt_brain::BrainError;
use systemprompt_extension::brain::RecallError;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_models::brain::{SubjectKeyError, UnknownFactStatus};
use systemprompt_traits::{AuthProviderError, RepositoryError};

use crate::error::ApiHttpError;

/// A brain route failure, rendered through the shared API error envelope.
#[derive(Debug)]
pub struct BrainHttpError(Box<ApiHttpError>);

impl BrainHttpError {
    #[must_use]
    pub fn into_inner(self) -> ApiError {
        (*self.0).into_inner()
    }
}

impl IntoResponse for BrainHttpError {
    fn into_response(self) -> Response {
        (*self.0).into_response()
    }
}

impl From<ApiHttpError> for BrainHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self(Box::new(error))
    }
}

impl From<ApiError> for BrainHttpError {
    fn from(error: ApiError) -> Self {
        Self(Box::new(ApiHttpError::from(error)))
    }
}

fn invalid(message: String, key: &'static str) -> ApiError {
    ApiError::validation_error(message, Vec::new()).with_error_key(key)
}

impl From<BrainError> for BrainHttpError {
    fn from(error: BrainError) -> Self {
        let api = match error {
            BrainError::NotFound(_) => {
                ApiError::not_found(error.to_string()).with_error_key("fact_not_found")
            },
            BrainError::ProposalNotFound(_) => {
                ApiError::not_found(error.to_string()).with_error_key("proposal_not_found")
            },
            BrainError::Forbidden { .. } => {
                ApiError::forbidden(error.to_string()).with_error_key("brain_forbidden")
            },
            BrainError::Tombstoned => {
                ApiError::conflict(error.to_string()).with_error_key("fact_tombstoned")
            },
            BrainError::UnknownKind(_) => invalid(error.to_string(), "unknown_kind"),
            BrainError::Subject(_) => invalid(error.to_string(), "invalid_subject"),
            BrainError::Statement(_) => invalid(error.to_string(), "invalid_statement"),
            BrainError::Repository(repo) => repository(repo),
            transient if transient.is_transient() => {
                ApiError::service_unavailable("brain storage is unavailable")
                    .with_error_key("brain_unavailable")
                    .with_source(transient)
            },
            other => ApiError::internal("brain operation failed", other),
        };
        Self::from(api)
    }
}

fn repository(error: RepositoryError) -> ApiError {
    match error {
        RepositoryError::NotFound { .. } => {
            ApiError::not_found(error.to_string()).with_error_key("fact_not_found")
        },
        RepositoryError::Conflict { .. } => {
            ApiError::conflict(error.to_string()).with_error_key("conflict")
        },
        RepositoryError::InvalidArgument { .. } | RepositoryError::InvalidData { .. } => {
            invalid(error.to_string(), "invalid_params")
        },
        other => ApiError::service_unavailable("brain storage is unavailable")
            .with_error_key("brain_unavailable")
            .with_source(other),
    }
}

impl From<AuthProviderError> for BrainHttpError {
    fn from(error: AuthProviderError) -> Self {
        ApiError::internal("user lookup failed", error).into()
    }
}

impl From<RecallError> for BrainHttpError {
    fn from(error: RecallError) -> Self {
        ApiError::service_unavailable("brain recall is unavailable")
            .with_error_key("recall_unavailable")
            .with_source(error)
            .into()
    }
}

impl From<SubjectKeyError> for BrainHttpError {
    fn from(error: SubjectKeyError) -> Self {
        invalid(error.to_string(), "invalid_subject").into()
    }
}

impl From<UnknownFactStatus> for BrainHttpError {
    fn from(error: UnknownFactStatus) -> Self {
        invalid(error.to_string(), "invalid_status").into()
    }
}

impl From<IdValidationError> for BrainHttpError {
    fn from(error: IdValidationError) -> Self {
        invalid(error.to_string(), "invalid_id").into()
    }
}
