//! # systemprompt-cloud
//!
//! Cloud API client, credentials management, OAuth login flow, and
//! tenant orchestration for systemprompt.io Cloud deployments. This
//! crate is the bridge between the local CLI/runtime and the
//! systemprompt.io control plane.
//!
//! ## Public surface
//!
//! - [`CloudApiClient`] — bearer-token-authenticated REST client.
//! - [`CloudCredentials`] / [`CredentialsBootstrap`] — on-disk and process-wide
//!   cloud credentials.
//! - [`StoredTenant`] / [`TenantStore`] — persistent tenants index.
//! - [`CliSession`] / [`SessionStore`] — multi-tenant CLI sessions.
//! - [`run_oauth_flow`] / [`run_checkout_callback_flow`] — browser-driven OAuth
//!   and Paddle checkout flows.
//! - [`wait_for_provisioning`] — SSE + polling watcher for tenant provisioning
//!   state.
//! - [`CloudPaths`] — XDG-aware discovery of credentials, sessions, tenants,
//!   and project files.
//! - [`profile_authoring`] — pure [`Profile`](systemprompt_models::Profile)
//!   construction for local and cloud deployment targets.
//! - [`deploy`] — Dockerfile rendering ([`DockerfileBuilder`]) and validation
//!   for the deployment image.
//! - [`secrets_env`] — deploy-time mapping of `secrets.json` to environment
//!   variables, including the signing-key PEM transport encoding.
//! - [`DockerCli`] — Docker invocations behind a [`CommandRunner`] seam.
//!
//! ## Errors
//!
//! All public APIs return [`CloudResult<T>`] (i.e.
//! `Result<T, CloudError>`). [`CloudError`] composes `reqwest`,
//! `std::io`, `serde_json`, and the more specific
//! [`CredentialsBootstrapError`] via `#[from]` so callers can use `?`
//! transparently.
//!
//! ## Feature flags
//!
//! This crate has no Cargo features — every dependency is required at
//! compile time. The `[package.metadata.docs.rs]` section in
//! `Cargo.toml` enables `all-features = true` for parity with the
//! rest of the workspace.

pub mod api_client;
pub mod auth;
pub mod checkout;
pub mod cli_session;
pub mod constants;
pub mod context;
pub mod credentials;
pub mod credentials_bootstrap;
pub mod deploy;
pub mod docker;
pub mod error;
pub mod oauth;
pub mod paths;
pub mod profile_authoring;
pub mod secrets_env;
pub mod tenants;

pub use api_client::{
    CheckoutEvent, CheckoutResponse, CloudApiClient, DeployResponse, ListSecretsResponse, Plan,
    ProvisioningEvent, ProvisioningEventType, RegistryToken, StatusResponse, SubscriptionStatus,
    Tenant, TenantInfo, TenantSecrets, TenantStatus, UserInfo, UserMeResponse,
};
pub use checkout::{
    CheckoutCallbackResult, CheckoutTemplates, run_checkout_callback_flow, wait_for_provisioning,
};
pub use cli_session::{CliSession, LOCAL_SESSION_KEY, SessionIdentity, SessionKey, SessionStore};
pub use constants::api::{PRODUCTION_URL, SANDBOX_URL};
pub use context::{CloudContext, ResolvedTenant};
pub use credentials::CloudCredentials;
pub use credentials_bootstrap::{CredentialsBootstrap, CredentialsBootstrapError};
pub use deploy::DockerfileBuilder;
pub use docker::{CommandRunner, CommandSpec, DockerCli, SystemCommandRunner};
pub use error::{CloudError, CloudResult};
pub use oauth::{OAuthTemplates, run_oauth_flow};
pub use paths::{
    CloudPath, CloudPaths, DiscoveredProject, ProfilePath, ProjectContext, ProjectPath,
    UnifiedContext, expand_home, get_cloud_paths, resolve_path,
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
