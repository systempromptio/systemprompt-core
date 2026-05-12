//! Public error type for `systemprompt-cloud`.
//!
//! All public APIs of this crate return [`CloudError`] (or
//! [`CloudResult<T>`]) instead of `anyhow::Error`. The enum is
//! `#[non_exhaustive]` so additional variants can be added in patch
//! releases without breaking downstream code that performs exhaustive
//! matching only on the documented variants.
//!
//! Upstream errors are composed via `#[from]` (`reqwest`, `std::io`,
//! `serde_json`) so callers can use `?` transparently.

use systemprompt_identifiers::TenantId;
use thiserror::Error;

mod messages;

pub type CloudResult<T> = Result<T, CloudError>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CloudError {
    #[error("Authentication required.\n\nRun: systemprompt cloud login")]
    NotAuthenticated,

    #[error("Token expired.\n\nRun: systemprompt cloud login")]
    TokenExpired,

    #[error("No tenant configured.\n\nRun: systemprompt cloud setup")]
    TenantNotConfigured,

    #[error("No app configured.\n\nRun: systemprompt cloud setup")]
    AppNotConfigured,

    #[error(
        "Profile required: {message}\n\nSet SYSTEMPROMPT_PROFILE or run 'systemprompt cloud \
         config'"
    )]
    ProfileRequired { message: String },

    #[error("Missing profile field: {field}\n\nAdd to your profile:\n{example}")]
    MissingProfileField { field: String, example: String },

    #[error("JWT decode error")]
    JwtDecode,

    #[error("Credentials file corrupted.\n\nRun: systemprompt cloud login")]
    CredentialsCorrupted {
        #[source]
        source: serde_json::Error,
    },

    #[error("Tenants not synced.\n\nRun: systemprompt cloud login")]
    TenantsNotSynced,

    #[error("Tenants store corrupted.\n\nRun: systemprompt cloud login")]
    TenantsStoreCorrupted {
        #[source]
        source: serde_json::Error,
    },

    #[error("Tenants store invalid: {message}")]
    TenantsStoreInvalid { message: String },

    #[error("Tenant '{}' not found.\n\nRun: systemprompt cloud config", tenant_id.as_str())]
    TenantNotFound { tenant_id: TenantId },

    #[error("API error: {message}")]
    ApiError { message: String },

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("Cloud API validation failed: {message}")]
    ApiValidationFailed { message: String },

    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials { message: String },

    #[error("Cloud credentials file not found: {path}")]
    CredentialsFileNotFound { path: String },

    #[error("Credentials not initialized")]
    CredentialsNotInitialized,

    #[error("Credentials already initialized")]
    CredentialsAlreadyInitialized,

    #[error(
        "Session file version mismatch: expected {min}-{max}, got {actual}. Delete {path} and \
         retry."
    )]
    SessionVersionMismatch {
        min: u32,
        max: u32,
        actual: u32,
        path: String,
    },

    #[error("OAuth flow failed: {message}")]
    OAuthFlow { message: String },

    #[error("Checkout flow failed: {message}")]
    CheckoutFlow { message: String },

    #[error("SSE stream error: {message}")]
    SseStream { message: String },

    #[error("Provisioning failed: {message}")]
    ProvisioningFailed { message: String },

    #[error("Authentication failed. Please run 'systemprompt cloud login' again.")]
    Unauthorized,

    #[error("Request failed with status {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("{message}")]
    Other { message: String },
}

impl CloudError {
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }

    pub const fn is_missing_credentials_file(&self) -> bool {
        matches!(self, Self::CredentialsFileNotFound { .. })
    }
}
