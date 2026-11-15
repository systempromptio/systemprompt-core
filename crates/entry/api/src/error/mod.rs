//! The API server's own handler error.
//!
//! Handlers in this crate return `Result<_, ApiHttpError>` and propagate
//! domain, repository and service errors with `?`; the variant-to-status
//! mapping lives once, in the `conversions` submodule's `From` impls. The
//! orphan rule requires the conversion target to be local to this crate, so
//! [`ApiHttpError`] is a thin newtype over the shared [`ApiError`] envelope,
//! and its response is built by the shared capability error
//! ([`systemprompt_extension::router::ApiHttpError`]) so the 5xx policy and
//! the logging have one home.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod conversions;

use axum::response::{IntoResponse, Response};
use systemprompt_extension::router::ApiHttpError as SharedHttpError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::BoxedSource;

pub use systemprompt_extension::router::unreadable_body;

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

impl IntoResponse for ApiHttpError {
    fn into_response(self) -> Response {
        SharedHttpError::from(*self.0).into_response()
    }
}
