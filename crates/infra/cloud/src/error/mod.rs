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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

mod messages;

pub type CloudResult<T> = Result<T, CloudError>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CloudError {
    #[error("Authentication required.\n\nRun: systemprompt cloud auth login")]
    NotAuthenticated,

    #[error("Token expired.\n\nRun: systemprompt cloud auth login")]
    TokenExpired,

    #[error("JWT decode error")]
    JwtDecode,

    #[error("Credentials file corrupted.\n\nRun: systemprompt cloud auth login")]
    CredentialsCorrupted {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("Tenants not synced.\n\nRun: systemprompt cloud auth login")]
    TenantsNotSynced,

    #[error("Tenants store corrupted.\n\nRun: systemprompt cloud auth login")]
    TenantsStoreCorrupted {
        #[source]
        source: serde_json::Error,
    },

    #[error("Tenant {tenant_id} is already in the tenant store")]
    TenantAlreadyExists {
        tenant_id: systemprompt_identifiers::TenantId,
    },

    #[error(
        "Tenant {tenant_id} is managed by systemprompt.io; it is synced from the control plane, \
         not added or removed locally"
    )]
    TenantNotSelfManaged {
        tenant_id: systemprompt_identifiers::TenantId,
    },

    #[error("Tenants store invalid: {source}")]
    TenantsStoreInvalid {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("API error ({status}) {code}: {message}")]
    ApiError {
        status: u16,
        code: String,
        message: String,
    },

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("Cloud API validation failed: {message}: {source}")]
    ApiValidationFailed {
        message: String,
        #[source]
        source: Box<Self>,
    },

    #[error("Cloud credentials file invalid: {source}")]
    InvalidCredentials {
        #[source]
        source: Box<Self>,
    },

    #[error("Cloud credentials file not found: {path}")]
    CredentialsFileNotFound { path: String },

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

    #[error(
        "Session store at {path} is stale or corrupt and cannot be parsed.\n\nRun: systemprompt \
         admin session switch <profile> to reset it"
    )]
    SessionStoreCorrupted {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("OAuth flow failed: {message}")]
    OAuthFlow { message: String },

    #[error("{message}")]
    Deploy {
        message: String,
        #[source]
        source: Option<DeploySource>,
    },

    #[error("{message}")]
    Dockerfile { message: String },

    #[error("{message}")]
    Docker {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    #[error("Authentication failed. Please run 'systemprompt cloud auth login' again.")]
    Unauthorized,

    #[error("Request failed with status {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error(
        "Provider '{provider}' has no deploy pipeline; deploy a docker_local tenant with \
         'systemprompt infra services start'"
    )]
    ProviderNotDeployable {
        provider: systemprompt_manifest::HostingProvider,
    },

    #[error("Profile has no cloud.deploy section; provider '{provider}' deploys through GitOps")]
    DeployNotConfigured {
        provider: systemprompt_manifest::HostingProvider,
    },

    #[error("{message}")]
    GitOps {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    #[error("Values file {path} could not be updated")]
    ValuesFile {
        path: String,
        #[source]
        source: crate::deploy::provider::ValuesFileError,
    },

    #[error("Tenant at {url} did not become ready within {waited_secs}s (last: {last})")]
    TenantUnready {
        url: String,
        waited_secs: u64,
        last: String,
    },

    #[error("Operator sign-in failed: {message}")]
    OperatorAuth { message: String },

    #[error("Operator issuer endpoint {endpoint} was refused")]
    OperatorEndpoint {
        endpoint: String,
        #[source]
        source: systemprompt_models::net::OutboundUrlError,
    },

    #[error("Operator ID token email is not a valid address")]
    OperatorEmail {
        #[source]
        source: systemprompt_identifiers::error::IdValidationError,
    },

    #[error(
        "No operator sign-in.\n\nRun: systemprompt cloud auth login --issuer <url> --client-id <id>"
    )]
    OperatorNotAuthenticated,
}

impl CloudError {
    pub fn deploy(message: impl Into<String>) -> Self {
        Self::Deploy {
            message: message.into(),
            source: None,
        }
    }

    pub fn deploy_with(message: impl Into<String>, source: impl Into<DeploySource>) -> Self {
        Self::Deploy {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    pub fn dockerfile(message: impl Into<String>) -> Self {
        Self::Dockerfile {
            message: message.into(),
        }
    }

    pub fn docker(message: impl Into<String>) -> Self {
        Self::Docker {
            message: message.into(),
            source: None,
        }
    }

    pub fn docker_with(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Docker {
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn gitops(message: impl Into<String>) -> Self {
        Self::GitOps {
            message: message.into(),
            source: None,
        }
    }

    pub fn gitops_with(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::GitOps {
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn operator_auth(message: impl Into<String>) -> Self {
        Self::OperatorAuth {
            message: message.into(),
        }
    }

    pub const fn is_missing_credentials_file(&self) -> bool {
        matches!(self, Self::CredentialsFileNotFound { .. })
    }

    pub const fn is_local_mode_recoverable(&self) -> bool {
        matches!(
            self,
            Self::CredentialsFileNotFound { .. }
                | Self::TokenExpired
                | Self::ApiValidationFailed { .. }
        )
    }
}

/// The underlying failure behind a `CloudError::Deploy`.
#[derive(Debug, thiserror::Error)]
pub enum DeploySource {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
