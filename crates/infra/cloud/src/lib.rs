#![allow(
    clippy::significant_drop_in_scrutinee,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::clone_on_ref_ptr,
    clippy::if_not_else,
    clippy::single_match_else,
    clippy::ignored_unit_patterns,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else
)]

pub mod api_client;
pub mod auth;
pub mod checkout;
pub mod cli_session;
pub mod constants;
pub mod context;
pub mod credentials;
pub mod credentials_bootstrap;
pub mod error;
pub mod oauth;
pub mod paths;
pub mod tenants;

pub use api_client::{
    CheckoutEvent, CheckoutResponse, CloudApiClient, DeployResponse, ListSecretsResponse, Plan,
    ProvisioningEvent, ProvisioningEventType, RegistryToken, StatusResponse, SubscriptionStatus,
    Tenant, TenantInfo, TenantSecrets, TenantStatus, UserInfo, UserMeResponse,
};
pub use checkout::{
    run_checkout_callback_flow, wait_for_provisioning, CheckoutCallbackResult, CheckoutTemplates,
};
pub use cli_session::CliSession;
pub use constants::api::{PRODUCTION_URL, SANDBOX_URL};
pub use context::{CloudContext, ResolvedTenant};
pub use credentials::CloudCredentials;
pub use credentials_bootstrap::{CredentialsBootstrap, CredentialsBootstrapError};
pub use error::{CloudError, CloudResult};
pub use oauth::{run_oauth_flow, OAuthTemplates};
pub use paths::{
    expand_home, get_cloud_paths, resolve_path, CloudPath, CloudPaths, DiscoveredProject,
    ProfilePath, ProjectContext, ProjectPath, UnifiedContext,
};
pub use tenants::{StoredTenant, TenantStore, TenantType};

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Environment {
    #[default]
    Production,
    Sandbox,
}

impl Environment {
    #[must_use]
    pub const fn api_url(&self) -> &'static str {
        match self {
            Self::Production => PRODUCTION_URL,
            Self::Sandbox => SANDBOX_URL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OAuthProvider {
    Github,
    Google,
}

impl OAuthProvider {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
        }
    }

    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Github => "GitHub",
            Self::Google => "Google",
        }
    }
}
