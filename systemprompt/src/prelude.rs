//! Prelude for extension authors.
//!
//! This module provides a unified set of imports for developing
//! SystemPrompt extensions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use systemprompt::prelude::*;
//! ```
//!
//! # What's Included
//!
//! ## Extension Framework
//! - `Extension` - The unified extension trait
//! - `ExtensionMetadata` - Extension identification
//! - `ExtensionRouter` - API route configuration
//! - `ExtensionContext` - Runtime context for extensions
//! - `ExtensionRegistry` - Extension discovery and management
//! - `SchemaDefinition` - Database schema definitions
//! - `register_extension!` - Macro for registering extensions
//!
//! ## Error Handling
//! - `ExtensionError` - Trait for extension errors
//! - `ApiError` - HTTP API error response
//! - `McpErrorData` - MCP protocol error format
//!
//! ## Jobs
//! - `Job` - Background job trait
//! - `JobContext` - Context passed to job execution
//! - `JobResult` - Result of job execution
//!
//! ## Repository (with `database` feature)
//! - `Entity` - Trait for database entities
//! - `EntityId` - Trait for entity ID types
//! - `GenericRepository` - Generic CRUD repository
//! - `RepositoryExt` - Extension methods for repositories
//!
//! ## MCP Tools (with `mcp` feature)
//! - Re-exports from `rmcp` crate for MCP server/tool development

// =============================================================================
// EXTENSION FRAMEWORK
// =============================================================================

#[cfg(feature = "core")]
pub use systemprompt_extension::{
    register_extension, Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry,
    ExtensionRouter, SchemaDefinition, SchemaSource,
};

#[cfg(feature = "core")]
pub use systemprompt_extension::error::{ConfigError, LoaderError};

// =============================================================================
// ERROR HANDLING
// =============================================================================

#[cfg(feature = "core")]
pub use systemprompt_traits::{ApiError, ExtensionError, McpErrorData};

// =============================================================================
// JOBS
// =============================================================================

#[cfg(feature = "core")]
pub use systemprompt_traits::{Job, JobContext, JobResult};

// =============================================================================
// PROVIDERS
// =============================================================================

#[cfg(feature = "core")]
pub use systemprompt_traits::{LlmProvider, LlmProviderResult, ToolProvider, ToolProviderResult};

// =============================================================================
// REPOSITORY (DATABASE FEATURE)
// =============================================================================

#[cfg(feature = "database")]
pub use systemprompt_core_database::{
    repository::{Entity, EntityId, GenericRepository, RepositoryExt},
    DatabaseProvider,
};

// =============================================================================
// SYSTEM CONTEXT (API FEATURE)
// =============================================================================

#[cfg(feature = "api")]
pub use systemprompt_runtime::{AppContext, AppContextBuilder};

// =============================================================================
// MCP (MCP FEATURE)
// =============================================================================

#[cfg(feature = "mcp")]
pub use rmcp;

// =============================================================================
// COMMON RE-EXPORTS
// =============================================================================

pub use std::sync::Arc;

#[cfg(feature = "api")]
pub use axum::Router;

#[cfg(feature = "database")]
pub use sqlx::PgPool;
