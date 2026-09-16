//! Application-owned orchestration over the managed marketplace domain:
//! source capture, inventory refresh and Git credential resolution. The domain
//! owns content and storage; this layer binds it to configured roots and
//! secrets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod capture;
pub mod git_sources;
pub mod inventory;

pub use capture::capture_authoring_input;

#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
    #[error("Source verification failed: {0}")]
    Source(String),
}
