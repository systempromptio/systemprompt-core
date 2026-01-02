use thiserror::Error;

#[derive(Debug, Error)]
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
        "Cloud features are disabled in this profile.\n\nSet cloud.cli_enabled: true in your \
         profile"
    )]
    CloudDisabled,

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

    #[error("Tenant '{tenant_id}' not found.\n\nRun: systemprompt cloud config")]
    TenantNotFound { tenant_id: String },

    #[error(transparent)]
    Network(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type CloudResult<T> = Result<T, CloudError>;

impl CloudError {
    pub const fn user_message(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "Not logged in to SystemPrompt Cloud",
            Self::TokenExpired => "Your session has expired",
            Self::TenantNotConfigured => "No project linked to this environment",
            Self::AppNotConfigured => "No deployment target configured",
            Self::CloudDisabled => "Cloud features are disabled in this profile",
            Self::ProfileRequired { .. } => "Profile configuration required",
            Self::MissingProfileField { .. } => "Missing required profile field",
            Self::JwtDecode => "Failed to decode authentication token",
            Self::CredentialsCorrupted { .. } => "Credentials file is corrupted",
            Self::TenantsNotSynced => "Tenants not synced locally",
            Self::TenantsStoreCorrupted { .. } => "Tenants store is corrupted",
            Self::TenantsStoreInvalid { .. } => "Tenants store is invalid",
            Self::TenantNotFound { .. } => "Tenant not found",
            Self::Network(_) => "Network error communicating with cloud",
            Self::Io(_) => "File system error",
        }
    }

    pub const fn recovery_hint(&self) -> &'static str {
        match self {
            Self::NotAuthenticated | Self::TokenExpired => {
                "Run 'systemprompt cloud login' to authenticate"
            },
            Self::TenantNotConfigured | Self::AppNotConfigured => {
                "Run 'systemprompt cloud setup' to configure your project"
            },
            Self::CloudDisabled => "Set cloud.cli_enabled: true in your profile YAML",
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
            Self::Network(_) => "Check your internet connection and try again",
            Self::Io(_) => "Check file permissions and disk space",
        }
    }

    pub const fn requires_login(&self) -> bool {
        matches!(
            self,
            Self::NotAuthenticated | Self::TokenExpired | Self::CredentialsCorrupted { .. }
        )
    }

    pub const fn requires_setup(&self) -> bool {
        matches!(self, Self::TenantNotConfigured | Self::AppNotConfigured)
    }
}
