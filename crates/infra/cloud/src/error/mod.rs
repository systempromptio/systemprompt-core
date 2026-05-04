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

use thiserror::Error;

mod messages;

/// Public result alias used by every public function in this crate.
pub type CloudResult<T> = Result<T, CloudError>;

/// Public error returned by every fallible public API in
/// `systemprompt-cloud`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CloudError {
    /// No credentials are present on disk.
    #[error("Authentication required.\n\nRun: systemprompt cloud login")]
    NotAuthenticated,

    /// Credentials exist but the embedded JWT has expired.
    #[error("Token expired.\n\nRun: systemprompt cloud login")]
    TokenExpired,

    /// The active profile has no `tenant` block.
    #[error("No tenant configured.\n\nRun: systemprompt cloud setup")]
    TenantNotConfigured,

    /// The active profile has no `app` block.
    #[error("No app configured.\n\nRun: systemprompt cloud setup")]
    AppNotConfigured,

    /// A profile is required but none is loaded.
    #[error(
        "Profile required: {message}\n\nSet SYSTEMPROMPT_PROFILE or run 'systemprompt cloud \
         config'"
    )]
    ProfileRequired {
        /// Human-readable description of the missing profile state.
        message: String,
    },

    /// A profile field required by this command is missing.
    #[error("Missing profile field: {field}\n\nAdd to your profile:\n{example}")]
    MissingProfileField {
        /// Profile field name (e.g. `tenant.id`).
        field: String,
        /// Inline YAML example showing the missing block.
        example: String,
    },

    /// Authenticated JWT could not be decoded.
    #[error("JWT decode error")]
    JwtDecode,

    /// Credentials JSON file is corrupted on disk.
    #[error("Credentials file corrupted.\n\nRun: systemprompt cloud login")]
    CredentialsCorrupted {
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },

    /// Tenants store is missing.
    #[error("Tenants not synced.\n\nRun: systemprompt cloud login")]
    TenantsNotSynced,

    /// Tenants store JSON is corrupted on disk.
    #[error("Tenants store corrupted.\n\nRun: systemprompt cloud login")]
    TenantsStoreCorrupted {
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },

    /// Tenants store deserialized but failed validator checks.
    #[error("Tenants store invalid: {message}")]
    TenantsStoreInvalid {
        /// Human-readable description of the validation failure.
        message: String,
    },

    /// A specific tenant is referenced but is not in the store.
    #[error("Tenant '{tenant_id}' not found.\n\nRun: systemprompt cloud config")]
    TenantNotFound {
        /// ID requested by the caller.
        tenant_id: String,
    },

    /// Cloud-side JSON-RPC returned a structured error.
    #[error("API error: {message}")]
    ApiError {
        /// Human-readable message extracted from the response.
        message: String,
    },

    /// HTTP request to the Cloud API failed at the transport layer.
    #[error(transparent)]
    Network(#[from] reqwest::Error),

    /// File system access error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization of an on-disk file failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Cloud API returned an error during credentials validation.
    #[error("Cloud API validation failed: {message}")]
    ApiValidationFailed {
        /// Underlying response body / parse error.
        message: String,
    },

    /// Persisted credentials failed structural validation.
    #[error("Cloud credentials file invalid: {message}")]
    InvalidCredentials {
        /// Validator output.
        message: String,
    },

    /// Persisted credentials file does not exist.
    #[error("Cloud credentials file not found: {path}")]
    CredentialsFileNotFound {
        /// Resolved on-disk path.
        path: String,
    },

    /// `CredentialsBootstrap` accessed before initialization.
    #[error("Credentials not initialized")]
    CredentialsNotInitialized,

    /// `CredentialsBootstrap::init` called twice.
    #[error("Credentials already initialized")]
    CredentialsAlreadyInitialized,

    /// CLI session file version does not match current expectations.
    #[error(
        "Session file version mismatch: expected {min}-{max}, got {actual}. Delete {path} and \
         retry."
    )]
    SessionVersionMismatch {
        /// Minimum supported version.
        min: u32,
        /// Maximum supported version.
        max: u32,
        /// Version observed on disk.
        actual: u32,
        /// Path that should be deleted before retrying.
        path: String,
    },

    /// OAuth callback flow timed out, was cancelled, or the embedded
    /// server stopped unexpectedly.
    #[error("OAuth flow failed: {message}")]
    OAuthFlow {
        /// Human-readable reason.
        message: String,
    },

    /// Paddle checkout callback flow timed out, was cancelled, or
    /// returned an explicit failure status.
    #[error("Checkout flow failed: {message}")]
    CheckoutFlow {
        /// Human-readable reason.
        message: String,
    },

    /// SSE stream error during `subscribe_*` events.
    #[error("SSE stream error: {message}")]
    SseStream {
        /// Underlying transport / parse failure.
        message: String,
    },

    /// Provisioning watcher reported a failure / timeout.
    #[error("Provisioning failed: {message}")]
    ProvisioningFailed {
        /// Reason emitted by the cloud provisioner.
        message: String,
    },

    /// HTTP request returned 401 Unauthorized.
    #[error("Authentication failed. Please run 'systemprompt cloud login' again.")]
    Unauthorized,

    /// HTTP request returned a non-success status with an unparsed
    /// body.
    #[error("Request failed with status {status}: {body}")]
    HttpStatus {
        /// HTTP status code.
        status: u16,
        /// Truncated response body.
        body: String,
    },

    /// Free-form cloud error with a context message.
    #[error("{message}")]
    Other {
        /// Human-readable message.
        message: String,
    },
}

impl CloudError {
    /// Build a [`CloudError::Other`] from any displayable value.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}
