//! [`CredentialsBootstrapError`] and its conversion into the
//! crate-level [`CloudError`].

use crate::error::CloudError;

/// Errors specific to credentials bootstrap state transitions.
///
/// Wide consumers should match on [`CloudError`] instead — this enum
/// is the precise shape used internally and is composed via
/// [`CloudError::from`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsBootstrapError {
    /// `get` was called before `CredentialsBootstrap::init`.
    #[error("Credentials not initialized")]
    NotInitialized,

    /// `init` was called twice.
    #[error("Credentials already initialized")]
    AlreadyInitialized,

    /// `require` was called but credentials are absent.
    #[error("Cloud credentials not available")]
    NotAvailable,

    /// Credentials file does not exist on disk.
    #[error("Cloud credentials file not found: {path}")]
    FileNotFound {
        /// Resolved on-disk path.
        path: String,
    },

    /// Credentials file failed structural validation.
    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials {
        /// Validator message.
        message: String,
    },

    /// JWT inside the credentials file has expired.
    #[error("Cloud token has expired. Run 'systemprompt cloud login' to refresh")]
    TokenExpired,

    /// Cloud API rejected the credentials at validation time.
    #[error("Cloud API validation failed: {message}")]
    ApiValidationFailed {
        /// API response body / parse error.
        message: String,
    },
}

impl From<CredentialsBootstrapError> for CloudError {
    fn from(value: CredentialsBootstrapError) -> Self {
        match value {
            CredentialsBootstrapError::NotInitialized => Self::CredentialsNotInitialized,
            CredentialsBootstrapError::AlreadyInitialized => Self::CredentialsAlreadyInitialized,
            CredentialsBootstrapError::NotAvailable => Self::NotAuthenticated,
            CredentialsBootstrapError::FileNotFound { path } => {
                Self::CredentialsFileNotFound { path }
            },
            CredentialsBootstrapError::InvalidCredentials { message } => {
                Self::InvalidCredentials { message }
            },
            CredentialsBootstrapError::TokenExpired => Self::TokenExpired,
            CredentialsBootstrapError::ApiValidationFailed { message } => {
                Self::ApiValidationFailed { message }
            },
        }
    }
}
