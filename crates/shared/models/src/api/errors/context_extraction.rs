//! The canonical `ContextExtractionError` → [`ApiError`] mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::execution::ContextExtractionError;

use super::ApiError;

impl From<ContextExtractionError> for ApiError {
    fn from(err: ContextExtractionError) -> Self {
        match err {
            ContextExtractionError::InvalidToken(source) => {
                Self::unauthorized("Invalid or expired token").with_source(source)
            },
            e @ (ContextExtractionError::MissingHeader { .. }
            | ContextExtractionError::MissingAuthHeader
            | ContextExtractionError::Revoked
            | ContextExtractionError::MissingSessionId
            | ContextExtractionError::MissingUserId) => Self::unauthorized(e.to_string()),
            e @ (ContextExtractionError::MissingContextId
            | ContextExtractionError::InvalidHeaderValue { .. }) => {
                Self::bad_request(e.to_string())
            },
            e @ ContextExtractionError::ForbiddenHeader { .. } => Self::forbidden(e.to_string()),
            e @ ContextExtractionError::UserNotFound { .. } => Self::not_found(e.to_string()),
            e @ ContextExtractionError::PayloadTooLarge { .. } => {
                Self::payload_too_large(e.to_string())
            },
            e @ ContextExtractionError::DatabaseError { .. } => {
                Self::internal("Request context lookup failed", e)
            },
        }
    }
}
