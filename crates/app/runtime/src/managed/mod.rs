//! Inventory orchestration over the managed marketplace domain.
//!
//! Inventory refresh binds the domain's inventory service to the configured
//! services root and the profile's services sources, and reads the managed
//! repository and the publish guard the `managed_resources` member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod inventory;

#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
    #[error("managed resources state is not provided")]
    State(#[from] systemprompt_extension::MissingDependency),
}
