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
    #[error("Git credential reference is unresolved")]
    CredentialUnresolved,
    #[error(
        "Git credential is stored under its bare label; the runtime resolves only {expected}. \
         Run `admin config secret migrate-git-credentials --apply`"
    )]
    CredentialNotMigrated { expected: String },
    #[error("Operation requires a registered Git source")]
    NotGitSource,
}
