//! [`CredentialsBootstrapError`] and its conversion into the
//! crate-level [`CloudError`].

use crate::error::CloudError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsBootstrapError {
    #[error("Credentials not initialized")]
    NotInitialized,

    #[error("Credentials already initialized")]
    AlreadyInitialized,

    #[error("Cloud credentials not available")]
    NotAvailable,

    #[error("Cloud credentials file not found: {path}")]
    FileNotFound { path: String },

    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials { message: String },

    #[error("Cloud token has expired. Run 'systemprompt cloud login' to refresh")]
    TokenExpired,

    #[error("Cloud API validation failed: {message}")]
    ApiValidationFailed { message: String },
}

impl CredentialsBootstrapError {
    pub const fn is_file_not_found(&self) -> bool {
        matches!(self, Self::FileNotFound { .. })
    }
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
