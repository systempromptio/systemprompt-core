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

#[cfg(feature = "core")]
pub use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    SchemaDefinition, register_extension,
};

#[cfg(feature = "core")]
pub use systemprompt_extension::error::{ConfigError, LoaderError};

#[cfg(feature = "core")]
pub use systemprompt_traits::{ApiError, ExtensionError, McpErrorData};

#[cfg(feature = "core")]
pub use systemprompt_traits::{Job, JobContext, JobResult};

#[cfg(feature = "core")]
pub use systemprompt_traits::{LlmProvider, LlmProviderResult, ToolProvider, ToolProviderResult};

#[cfg(feature = "database")]
pub use systemprompt_database::{DatabaseProvider, DbPool};

#[cfg(feature = "api")]
pub use systemprompt_runtime::{AppContext, AppContextBuilder};

#[cfg(feature = "mcp")]
pub use rmcp;

#[cfg(feature = "full")]
pub use systemprompt_loader::ConfigLoader;

#[cfg(feature = "full")]
pub use systemprompt_logging::init_logging;

#[cfg(feature = "full")]
pub use systemprompt_mcp::{McpHttpConfig, SessionTimeouts, create_router as create_mcp_router};

pub use std::sync::Arc;

#[cfg(feature = "api")]
pub use axum::Router;

#[cfg(feature = "database")]
pub use sqlx::PgPool;
