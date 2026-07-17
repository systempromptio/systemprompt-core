//! Compile-time-checked sub-traits for typed extension declarations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod api;
mod config;
mod job;
mod provider;
mod schema;

pub use api::{ApiExtensionTyped, ApiExtensionTypedDyn};
pub use config::ConfigExtensionTyped;
pub use job::JobExtensionTyped;
pub use provider::ProviderExtensionTyped;
pub use schema::{SchemaDefinitionTyped, SchemaExtensionTyped};
