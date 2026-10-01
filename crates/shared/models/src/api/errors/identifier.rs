//! The canonical `IdValidationError` → [`ApiError`] mapping.
//!
//! An identifier that fails to parse at an HTTP edge (a path segment, a
//! query parameter, a body field) answers 400 naming the identifier type,
//! with the stable `invalid_identifier` key. It never becomes a fallback id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;

use super::ApiError;

impl From<IdValidationError> for ApiError {
    fn from(err: IdValidationError) -> Self {
        Self::bad_request(err.to_string()).with_error_key("invalid_identifier")
    }
}
