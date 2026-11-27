//! [`MobileError`], the one error type that crosses the binding boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_client::ClientError;
use uuid::Uuid;

/// What a native app can act on. An internal failure carries a correlation id
/// that names the logged cause; the cause itself stays on the Rust side.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    #[error("the address does not name an instance")]
    InvalidAddress,
    #[error("the instance could not be reached")]
    Network,
    #[error("the instance does not offer this")]
    NotFound,
    #[error("the instance refused the request with status {status}")]
    Refused { status: u16 },
    #[error("this build is too old for the instance")]
    VersionTooOld,
    #[error("internal error {correlation_id}")]
    Internal { correlation_id: String },
}

impl MobileError {
    fn internal(cause: &dyn std::error::Error) -> Self {
        let correlation_id = Uuid::new_v4().to_string();
        tracing::error!(%correlation_id, error = %cause, "mobile binding call failed");
        Self::Internal { correlation_id }
    }

    const fn from_status(status: u16) -> Self {
        match status {
            404 => Self::NotFound,
            _ => Self::Refused { status },
        }
    }
}

impl From<ClientError> for MobileError {
    fn from(error: ClientError) -> Self {
        match &error {
            ClientError::HttpError(source) if source.is_decode() || source.is_builder() => {
                Self::internal(&error)
            },
            ClientError::Timeout | ClientError::HttpError(_) => Self::Network,
            ClientError::ApiError { status, .. } | ClientError::UnreadableErrorBody { status, .. } => {
                Self::from_status(*status)
            },
            ClientError::JsonError(_)
            | ClientError::AuthError { .. }
            | ClientError::StreamEndedWithoutExitCode
            | ClientError::UndecodableEvent(_)
            | ClientError::Io(_) => Self::internal(&error),
        }
    }
}
