#[cfg(feature = "core")]
pub use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
    SchemaDefinition, SchemaSource, register_extension,
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
pub use systemprompt_database::{
    DatabaseProvider, DbPool,
    repository::{Entity, EntityId, GenericRepository, RepositoryExt},
};

#[cfg(feature = "api")]
pub use systemprompt_runtime::{AppContext, AppContextBuilder};

#[cfg(feature = "mcp")]
pub use rmcp;

#[cfg(feature = "full")]
pub use systemprompt_loader::ConfigLoader;

#[cfg(feature = "full")]
pub use systemprompt_logging::init_logging;

#[cfg(feature = "full")]
pub use systemprompt_mcp::create_router as create_mcp_router;

pub use std::sync::Arc;

#[cfg(feature = "api")]
pub use axum::Router;

#[cfg(feature = "database")]
pub use sqlx::PgPool;
