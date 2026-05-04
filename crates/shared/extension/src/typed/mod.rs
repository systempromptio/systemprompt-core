//! Compile-time-checked sub-traits for typed extension declarations.

mod api;
mod config;
mod job;
mod provider;
mod schema;

/// Typed contract for an extension that mounts an axum router.
pub use api::{ApiExtensionTyped, ApiExtensionTypedDyn};
/// Typed contract for an extension that owns a configuration namespace.
pub use config::ConfigExtensionTyped;
/// Typed contract for an extension that contributes scheduled jobs.
pub use job::JobExtensionTyped;
/// Typed contract for an extension that contributes provider
/// implementations.
pub use provider::ProviderExtensionTyped;
/// Typed schema definition value type and its source enum.
pub use schema::{SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped};
