//! User-facing message + recovery-hint helpers for [`super::CloudError`].

use super::CloudError;

impl CloudError {
    /// Short user-facing description of the error category.
    pub const fn user_message(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "Not logged in to systemprompt.io Cloud",
            Self::TokenExpired => "Your session has expired",
            Self::TenantNotConfigured => "No project linked to this environment",
            Self::AppNotConfigured => "No deployment target configured",
            Self::ProfileRequired { .. } => "Profile configuration required",
            Self::MissingProfileField { .. } => "Missing required profile field",
            Self::JwtDecode => "Failed to decode authentication token",
            Self::CredentialsCorrupted { .. } => "Credentials file is corrupted",
            Self::TenantsNotSynced => "Tenants not synced locally",
            Self::TenantsStoreCorrupted { .. } => "Tenants store is corrupted",
            Self::TenantsStoreInvalid { .. } => "Tenants store is invalid",
            Self::TenantNotFound { .. } => "Tenant not found",
            Self::ApiError { .. } => "API request failed",
            Self::Network(_) => "Network error communicating with cloud",
            Self::Io(_) => "File system error",
            Self::Json(_) => "JSON parse error",
            Self::ApiValidationFailed { .. } => "Cloud API rejected credentials",
            Self::InvalidCredentials { .. } => "Stored credentials are invalid",
            Self::CredentialsFileNotFound { .. } => "Credentials file is missing",
            Self::CredentialsNotInitialized => "Credentials bootstrap not initialised",
            Self::CredentialsAlreadyInitialized => "Credentials bootstrap already initialised",
            Self::SessionVersionMismatch { .. } => "CLI session file is out of date",
            Self::OAuthFlow { .. } => "OAuth login flow failed",
            Self::CheckoutFlow { .. } => "Cloud checkout flow failed",
            Self::SseStream { .. } => "Cloud SSE stream failed",
            Self::ProvisioningFailed { .. } => "Tenant provisioning failed",
            Self::Unauthorized => "Cloud API rejected this token",
            Self::HttpStatus { .. } => "Cloud API returned a non-success status",
            Self::Other { .. } => "Cloud operation failed",
        }
    }

    /// Recovery hint surfaced by the CLI.
    pub const fn recovery_hint(&self) -> &'static str {
        match self {
            Self::NotAuthenticated | Self::TokenExpired | Self::Unauthorized => {
                "Run 'systemprompt cloud login' to authenticate"
            },
            Self::TenantNotConfigured | Self::AppNotConfigured => {
                "Run 'systemprompt cloud setup' to configure your project"
            },
            Self::ProfileRequired { .. } => {
                "Set SYSTEMPROMPT_PROFILE or run 'systemprompt cloud config'"
            },
            Self::MissingProfileField { .. } => "Add the missing field to your profile YAML",
            Self::JwtDecode | Self::CredentialsCorrupted { .. } => {
                "Run 'systemprompt cloud login' to re-authenticate"
            },
            Self::TenantsNotSynced
            | Self::TenantsStoreCorrupted { .. }
            | Self::TenantsStoreInvalid { .. } => "Run 'systemprompt cloud login' to sync tenants",
            Self::TenantNotFound { .. } => {
                "Run 'systemprompt cloud config' to select a valid tenant"
            },
            Self::ApiError { .. } | Self::HttpStatus { .. } | Self::ApiValidationFailed { .. } => {
                "Check the error message and try again"
            },
            Self::Network(_) => "Check your internet connection and try again",
            Self::Io(_) => "Check file permissions and disk space",
            Self::Json(_) => "Inspect the JSON file referenced in the error message",
            Self::InvalidCredentials { .. } | Self::CredentialsFileNotFound { .. } => {
                "Run 'systemprompt cloud login' to refresh credentials"
            },
            Self::CredentialsNotInitialized | Self::CredentialsAlreadyInitialized => {
                "Restart the process to re-run the bootstrap sequence"
            },
            Self::SessionVersionMismatch { .. } => "Delete the session file and re-authenticate",
            Self::OAuthFlow { .. } => "Re-run the OAuth login flow",
            Self::CheckoutFlow { .. } => "Re-run the checkout flow",
            Self::SseStream { .. } => "Retry the operation; the server falls back to polling",
            Self::ProvisioningFailed { .. } => "Inspect 'systemprompt cloud status' for details",
            Self::Other { .. } => "Inspect the error message and try again",
        }
    }

    /// `true` when the user must run `systemprompt cloud login`.
    pub const fn requires_login(&self) -> bool {
        matches!(
            self,
            Self::NotAuthenticated
                | Self::TokenExpired
                | Self::CredentialsCorrupted { .. }
                | Self::Unauthorized
        )
    }

    /// `true` when the user must run `systemprompt cloud setup`.
    pub const fn requires_setup(&self) -> bool {
        matches!(self, Self::TenantNotConfigured | Self::AppNotConfigured)
    }
}
