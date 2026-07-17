//! Curated re-exports for `use systemprompt::prelude::*`.
//!
//! The prelude is intentionally narrow — it covers the types that almost every
//! consumer needs (extension trait surface, errors, common providers, and a
//! small set of re-exported third-party types when their feature is enabled).
//! Anything specific to a single domain lives behind its module path
//! (`systemprompt::agent::…`, `systemprompt::ai::…`, etc.).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Core extension framework — the `Extension` trait, typed metadata, the
/// `register_extension!` macro, and registry/router types.
#[cfg(feature = "core")]
pub use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    SchemaDefinition, register_extension,
};

/// Typed extension errors — `ConfigError` (extension config validation) and
/// `LoaderError` (extension discovery / load failures).
#[cfg(feature = "core")]
pub use systemprompt_extension::error::{ConfigError, LoaderError};

/// Cross-cutting trait errors — `ApiError` for HTTP boundaries,
/// `ExtensionError` for extension lifecycle, `McpErrorData` for MCP responses.
#[cfg(feature = "core")]
pub use systemprompt_traits::{ApiError, ExtensionError, McpErrorData};

/// Job trait surface — `Job`, `JobContext`, `JobResult` for implementing
/// scheduled or background work.
#[cfg(feature = "core")]
pub use systemprompt_traits::{Job, JobContext, JobResult};

/// Provider trait surface — `LlmProvider` and `ToolProvider` plus their typed
/// `Result` aliases. Implement these to plug a custom inference backend or
/// tool host into the platform.
#[cfg(feature = "core")]
pub use systemprompt_traits::{LlmProvider, LlmProviderResult, ToolProvider, ToolProviderResult};

/// Database surface — `DbPool` (the workspace-wide SQLx pool wrapper) and
/// `DatabaseProvider` (the trait object used for runtime injection).
#[cfg(feature = "database")]
pub use systemprompt_database::{DatabaseProvider, DbPool};

/// Application-runtime surface — `AppContext` (the read-only handle services
/// hold) and `AppContextBuilder` (its construction-time mutable form).
#[cfg(feature = "api")]
pub use systemprompt_runtime::{AppContext, AppContextBuilder};

/// Re-export of the upstream `rmcp` crate so consumers can derive MCP tool
/// types via the same macros the platform itself uses.
#[cfg(feature = "mcp")]
pub use rmcp;

/// Filesystem-backed config loader from `systemprompt-loader`.
#[cfg(feature = "full")]
pub use systemprompt_loader::ConfigLoader;

/// Tracing initialisation entry point from `systemprompt-logging`.
#[cfg(feature = "full")]
pub use systemprompt_logging::init_logging;

#[cfg(feature = "full")]
pub use systemprompt_mcp::{McpHttpConfig, SessionTimeouts, create_router as create_mcp_router};

/// Convenience re-export of `std::sync::Arc` so consumers do not need a
/// separate `use std::sync::Arc` line when wiring extensions.
pub use std::sync::Arc;

/// Axum `Router` type — re-exported so consumers writing `ApiExtensionTyped`
/// implementations do not need a direct `axum` dependency.
#[cfg(feature = "api")]
pub use axum::Router;

/// SQLx Postgres pool — re-exported for consumers building repositories on
/// top of the `database` feature.
#[cfg(feature = "database")]
pub use sqlx::PgPool;
