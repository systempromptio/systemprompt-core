//! User-facing message + recovery-hint helpers for [`super::CloudError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CloudError;

impl CloudError {
    pub const fn user_message(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "Not logged in to systemprompt.io Cloud",
            Self::TokenExpired => "Your session has expired",
            Self::JwtDecode => "Failed to decode authentication token",
            Self::CredentialsCorrupted { .. } => "Credentials file is corrupted",
            Self::TenantsNotSynced => "Tenants not synced locally",
            Self::TenantsStoreCorrupted { .. } => "Tenants store is corrupted",
            Self::TenantsStoreInvalid { .. } => "Tenants store is invalid",
            Self::TenantAlreadyExists { .. } => "Tenant already exists",
            Self::TenantNotSelfManaged { .. } => "Tenant is managed by systemprompt.io",
            Self::ApiError { .. } => "API request failed",
            Self::Network(_) => "Network error communicating with cloud",
            Self::Io(_) => "File system error",
            Self::Json(_) => "JSON parse error",
            Self::ApiValidationFailed { .. } => "Cloud API rejected credentials",
            Self::InvalidCredentials { .. } => "Stored credentials are invalid",
            Self::CredentialsFileNotFound { .. } => "Credentials file is missing",
            Self::SessionVersionMismatch { .. } => "CLI session file is out of date",
            Self::SessionStoreCorrupted { .. } => "CLI session store is stale or corrupt",
            Self::OAuthFlow { .. } => "OAuth login flow failed",
            Self::Deploy { .. } => "Cloud deploy failed",
            Self::Dockerfile { .. } => "Dockerfile validation failed",
            Self::Docker { .. } => "Docker command failed",
            Self::Unauthorized => "Cloud API rejected this token",
            Self::HttpStatus { .. } => "Cloud API returned a non-success status",
            Self::ProviderNotDeployable { .. } => "This provider has no deploy pipeline",
            Self::DeployNotConfigured { .. } => "Deploy target is not configured",
            Self::GitOps { .. } => "GitOps update failed",
            Self::ValuesFile { .. } => "Values file could not be updated",
            Self::TenantUnready { .. } => "Tenant did not become ready",
            Self::OperatorAuth { .. }
            | Self::OperatorEndpoint { .. }
            | Self::OperatorEmail { .. } => "Operator sign-in failed",
            Self::OperatorNotAuthenticated => "Not signed in as an operator",
        }
    }

    pub const fn recovery_hint(&self) -> &'static str {
        match self {
            Self::NotAuthenticated | Self::TokenExpired | Self::Unauthorized => {
                "Run 'systemprompt cloud auth login' to authenticate"
            },
            Self::JwtDecode | Self::CredentialsCorrupted { .. } => {
                "Run 'systemprompt cloud auth login' to re-authenticate"
            },
            Self::TenantsNotSynced
            | Self::TenantsStoreCorrupted { .. }
            | Self::TenantsStoreInvalid { .. } => {
                "Run 'systemprompt cloud auth login' to sync tenants"
            },
            Self::TenantAlreadyExists { .. } => {
                "Choose another id, or remove the existing record first"
            },
            Self::TenantNotSelfManaged { .. } => {
                "Use 'systemprompt cloud tenant delete' for systemprompt.io tenants"
            },
            Self::ApiError { .. } | Self::HttpStatus { .. } | Self::ApiValidationFailed { .. } => {
                "Check the error message and try again"
            },
            Self::Network(_) => "Check your internet connection and try again",
            Self::Io(_) => "Check file permissions and disk space",
            Self::Json(_) => "Inspect the JSON file referenced in the error message",
            Self::InvalidCredentials { .. } | Self::CredentialsFileNotFound { .. } => {
                "Run 'systemprompt cloud auth login' to refresh credentials"
            },
            Self::SessionVersionMismatch { .. } => "Delete the session file and re-authenticate",
            Self::SessionStoreCorrupted { .. } => {
                "Run 'systemprompt admin session switch <profile>' to reset the session store"
            },
            Self::OAuthFlow { .. } => "Re-run the OAuth login flow",
            Self::Dockerfile { .. } => {
                "Regenerate the Dockerfile with 'systemprompt cloud profile create'"
            },
            Self::Docker { .. } => "Check that Docker is installed and running",
            Self::Deploy { .. } => "Inspect the error message and try again",
            Self::ProviderNotDeployable { .. } => {
                "Run the tenant locally with 'systemprompt infra services start'"
            },
            Self::DeployNotConfigured { .. } => {
                "Add cloud.deploy (registry, repository, gitops) to the profile"
            },
            Self::GitOps { .. } | Self::ValuesFile { .. } => {
                "Check the ops repository working copy and cloud.deploy.gitops settings"
            },
            Self::TenantUnready { .. } => {
                "Check the rollout in the cluster, then run 'systemprompt cloud status'"
            },
            Self::OperatorAuth { .. }
            | Self::OperatorEndpoint { .. }
            | Self::OperatorEmail { .. }
            | Self::OperatorNotAuthenticated => {
                "Run 'systemprompt cloud auth login --issuer <url> --client-id <id>'"
            },
        }
    }

    pub const fn requires_login(&self) -> bool {
        matches!(
            self,
            Self::NotAuthenticated
                | Self::TokenExpired
                | Self::CredentialsCorrupted { .. }
                | Self::Unauthorized
        )
    }
}
