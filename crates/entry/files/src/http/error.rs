//! The handler error of the files routes: a thin newtype over the shared
//! [`ApiError`] envelope that renders through the composition contract's
//! [`ApiHttpError`](systemprompt_extension::router::ApiHttpError), so the 5xx
//! policy keeps one home. It carries the `From` mappings of the errors the
//! upload, issue and sharing handlers propagate with `?`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::{IntoResponse, Response};
use systemprompt_documents::http::error::DocumentsHttpError;
use systemprompt_extension::router::ApiHttpError as SharedHttpError;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::BoxedSource;
use systemprompt_users::UserError;

#[derive(Debug)]
pub struct ApiHttpError(Box<ApiError>);

impl ApiHttpError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self(Box::new(ApiError::not_found(message)))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self(Box::new(ApiError::bad_request(message)))
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self(Box::new(ApiError::unauthorized(message)))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self(Box::new(ApiError::forbidden(message)))
    }

    pub fn internal(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self(Box::new(ApiError::internal(context, source)))
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self(Box::new(ApiError::conflict(message)))
    }

    pub fn api(&self) -> &ApiError {
        &self.0
    }

    pub fn into_inner(self) -> ApiError {
        *self.0
    }
}

impl From<ApiError> for ApiHttpError {
    fn from(error: ApiError) -> Self {
        Self(Box::new(error))
    }
}

impl From<IdValidationError> for ApiHttpError {
    fn from(error: IdValidationError) -> Self {
        Self(Box::new(ApiError::from(error)))
    }
}

impl From<UserError> for ApiHttpError {
    fn from(error: UserError) -> Self {
        let api = match error {
            UserError::Repository(inner) => ApiError::from(inner),
            e @ UserError::NotFound(_) => ApiError::not_found(e.to_string()),
            e @ UserError::FederatedRefused(_) => ApiError::forbidden(e.to_string()),
            e @ (UserError::EmailAlreadyExists { .. }
            | UserError::AccountKindMismatch { .. }
            | UserError::LegalHold(_)
            | UserError::NotArchived(_)
            | UserError::RestoreRefused { .. }) => ApiError::conflict(e.to_string()),
            e @ (UserError::Validation(_)
            | UserError::InvalidRoles(_)
            | UserError::ApiKeyLifetimeExceeded { .. }) => ApiError::bad_request(e.to_string()),
            e @ (UserError::MergeUnavailable
            | UserError::SecurityTableReassignment { .. }
            | UserError::PurgeIdentifier { .. }
            | UserError::OwnerReassignment { .. }) => {
                ApiError::internal("User operation failed", e)
            },
        };
        Self(Box::new(api))
    }
}

impl From<ApiHttpError> for DocumentsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::from(error.into_inner())
    }
}

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        SharedHttpError::from(*self.0).into_response()
    }
}
