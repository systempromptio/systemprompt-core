//! Rendering an [`ExtensionError`] as an [`ApiError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::ExtensionError;

use super::{ApiError, ErrorCode};

const EXTENSION_FAILURE: &str = "Extension operation failed";

impl ApiError {
    pub fn from_extension<E: ExtensionError>(err: E) -> Self {
        let code = match err.status().as_u16() {
            400 => ErrorCode::BadRequest,
            401 => ErrorCode::Unauthorized,
            403 => ErrorCode::Forbidden,
            404 => ErrorCode::NotFound,
            409 => ErrorCode::ConflictError,
            422 => ErrorCode::ValidationError,
            429 => ErrorCode::RateLimited,
            503 => ErrorCode::ServiceUnavailable,
            status if status >= 500 => ErrorCode::InternalError,
            _ => ErrorCode::BadRequest,
        };
        let key = err.code();
        if code.is_server_error() {
            return Self::new(code, EXTENSION_FAILURE)
                .with_error_key(key)
                .with_source(err);
        }
        Self::new(code, err.user_message())
            .with_error_key(key)
            .with_source(err)
    }
}
