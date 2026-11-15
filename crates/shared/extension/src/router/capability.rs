//! The one way a capability's entry crate answers its own errors over HTTP.
//!
//! The orphan rule forbids an entry crate from writing
//! `impl From<ItsDomainError> for ApiHttpError`: both types are foreign to
//! it. An entry crate therefore defines one local error enum — its domain
//! errors as `#[from]` variants plus an `Http(ApiHttpError)` variant, with
//! its `From<ApiHttpError>`, for failures the API already models —
//! implements [`IntoApiError`] for it (the status, `error_key` and message of
//! each variant), and invokes `capability_error_response!(LocalError)` once.
//! Handlers return `Result<_, LocalError>` and keep bare `?`.
//!
//! The response is always built by [`ApiHttpError`]'s `IntoResponse`, so the
//! 5xx policy has one home: a 5xx body is the fixed public message of its
//! code, the cause chain is logged once as a structured field, and it is never
//! serialised. An `IntoApiError` impl builds a 5xx with
//! `ApiError::internal(context, source)` and keeps the cause as its source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;

use super::ApiHttpError;

/// A capability error's mapping onto the API error envelope.
///
/// Implemented by each entry crate for its own error type; the response is
/// built by [`capability_response`], never by the implementor.
pub trait IntoApiError {
    fn into_api_error(self) -> ApiError;
}

impl IntoApiError for ApiError {
    fn into_api_error(self) -> ApiError {
        self
    }
}

impl IntoApiError for ApiHttpError {
    fn into_api_error(self) -> ApiError {
        self.into_inner()
    }
}

pub fn capability_response(error: impl IntoApiError) -> Response {
    ApiHttpError::from(error.into_api_error()).into_response()
}

#[macro_export]
macro_rules! capability_error_response {
    ($error:ty) => {
        impl $crate::kernel::__IntoResponse for $error {
            fn into_response(self) -> $crate::kernel::__Response {
                $crate::kernel::capability_response(self)
            }
        }
    };
}
