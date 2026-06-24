//! Entry-local HTTP error type for non-OAuth API routes.
//!
//! Handlers return `Result<_, ApiHttpError>` and propagate domain, repository,
//! and service errors with `?`. The variant-to-HTTP-status mapping lives once,
//! in the `conversions` submodule's `From` impls, so `domain/*` never
//! references the HTTP envelope and the boundary decides the status code from
//! the error variant rather than at each call site.
//!
//! The wire shape is the shared [`ApiError`] JSON envelope; [`ApiHttpError`] is
//! a thin entry-local newtype whose only reason to exist is the orphan rule —
//! `impl From<DomainError> for ApiError` is forbidden in this crate (both types
//! are foreign), so a local target type is required to obtain bare `?`.
//! `into_response` delegates to `ApiError`, which logs exactly once by status
//! class.

mod conversions;

use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;

#[derive(Debug)]
pub struct ApiHttpError(ApiError);

impl ApiHttpError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self(ApiError::not_found(message))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self(ApiError::bad_request(message))
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self(ApiError::unauthorized(message))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self(ApiError::forbidden(message))
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self(ApiError::internal_error(message))
    }

    pub fn into_inner(self) -> ApiError {
        self.0
    }
}

impl From<ApiError> for ApiHttpError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}
