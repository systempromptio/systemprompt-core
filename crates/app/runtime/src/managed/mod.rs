//! Application-owned orchestration over the managed marketplace domain.
//!
//! Inventory refresh and Git credential resolution. The domain owns content
//! and storage; this layer binds it to configured roots and secrets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod git_sources;
pub mod inventory;

#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
    #[error("Configured inventory could not be loaded; previous inventory retained: {0}")]
    InventoryLoad(#[source] systemprompt_loader::ConfigLoadError),
    #[error("Git credentials are unavailable: {0}")]
    CredentialsUnavailable(#[source] systemprompt_config::SecretsBootstrapError),
    #[error("Git credential reference is unresolved")]
    CredentialUnresolved,
    #[error("Operation requires a registered Git source")]
    NotGitSource,
}
