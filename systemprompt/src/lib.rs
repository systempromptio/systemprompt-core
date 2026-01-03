#![allow(clippy::doc_markdown)]

//! SystemPrompt - Extensible AI agent orchestration framework
//!
//! This crate provides a unified API for building and extending SystemPrompt
//! applications. It re-exports the core libraries with feature flags for
//! customization.
//!
//! # Features
//!
//! - `core` (default): Basic types, traits, and extension framework
//! - `database`: Database abstraction and repository patterns
//! - `api`: HTTP API server functionality
//! - `full`: Everything including all domain modules
//!
//! # Quick Start
//!
//! ```toml
//! [dependencies]
//! systemprompt = { version = "0.1", features = ["api"] }
//! ```
//!
//! # Creating Extensions
//!
//! Extensions allow you to add custom functionality to SystemPrompt.
//! Use the unified `Extension` trait with optional capabilities:
//!
//! ```rust,ignore
//! use systemprompt::prelude::*;
//!
//! #[derive(Default)]
//! struct MyExtension;
//!
//! impl Extension for MyExtension {
//!     fn metadata(&self) -> ExtensionMetadata {
//!         ExtensionMetadata {
//!             id: "my-extension",
//!             name: "My Extension",
//!             version: "1.0.0",
//!         }
//!     }
//!
//!     // Optional: provide API routes
//!     fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
//!         Some(ExtensionRouter::new(
//!             axum::Router::new()
//!                 .route("/hello", axum::routing::get(|| async { "Hello!" })),
//!             "/api/v1/my-ext",
//!         ))
//!     }
//!
//!     // Optional: provide database schemas
//!     fn schemas(&self) -> Vec<SchemaDefinition> {
//!         vec![]
//!     }
//! }
//!
//! register_extension!(MyExtension);
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

// =============================================================================
// CORE EXPORTS
// =============================================================================

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod traits {
    //! Core traits for providers and services.
    pub use systemprompt_traits::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod models {
    //! Data models and configuration types.
    pub use systemprompt_models::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod identifiers {
    //! Strongly-typed identifiers.
    pub use systemprompt_identifiers::*;
}

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod extension {
    //! Extension framework for custom modules.
    pub use systemprompt_extension::*;
}

// =============================================================================
// DATABASE EXPORTS
// =============================================================================

#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
pub mod database {
    //! Database abstraction and repository patterns.
    pub use systemprompt_core_database::*;
}

// =============================================================================
// INFRASTRUCTURE EXPORTS (full feature)
// =============================================================================

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod logging {
    //! Logging and observability.
    pub use systemprompt_core_logging::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod config {
    //! Configuration management.
    pub use systemprompt_core_config::*;
}

// =============================================================================
// API EXPORTS
// =============================================================================

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod system {
    //! Core system module with AppContext and registration.
    pub use systemprompt_runtime::*;
}

#[cfg(feature = "api")]
#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api {
    //! HTTP API server functionality.
    pub use systemprompt_core_api::*;
}

// =============================================================================
// CLI EXPORTS
// =============================================================================

#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli {
    //! CLI entry point for product binaries.
    pub use systemprompt_cli::run;
    pub use systemprompt_cli::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
}

// =============================================================================
// DOMAIN EXPORTS (full feature)
// =============================================================================

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod agent {
    //! A2A protocol agent functionality.
    pub use systemprompt_core_agent::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod ai {
    //! AI services and providers.
    pub use systemprompt_core_ai::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod mcp {
    //! MCP (Model Context Protocol) server support.
    pub use systemprompt_core_mcp::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod oauth {
    //! OAuth2/OIDC authentication.
    pub use systemprompt_core_oauth::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod users {
    //! User management.
    pub use systemprompt_core_users::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod content {
    //! Content management (blog/pages).
    pub use systemprompt_core_content::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod analytics {
    //! Analytics and metrics.
    pub use systemprompt_core_analytics::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod scheduler {
    //! Job scheduling.
    pub use systemprompt_core_scheduler::*;
}

#[cfg(feature = "full")]
#[cfg_attr(docsrs, doc(cfg(feature = "full")))]
pub mod files {
    //! File management.
    pub use systemprompt_core_files::*;
}

// =============================================================================
// SYNC & CLOUD EXPORTS
// =============================================================================

#[cfg(feature = "sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
pub mod sync {
    //! Synchronization services for cloud operations.
    //!
    //! Provides file sync, database sync, content sync, and deployment
    //! services.
    pub use systemprompt_sync::*;
}

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod cloud {
    //! Cloud infrastructure (API client, credentials, OAuth).
    //!
    //! Provides cloud API client, credential management, and authentication
    //! flows.
    pub use systemprompt_cloud::*;
}

// =============================================================================
// PROFILE BOOTSTRAP
// =============================================================================

#[cfg(feature = "core")]
#[cfg_attr(docsrs, doc(cfg(feature = "core")))]
pub mod profile {
    //! Profile bootstrap system for application configuration.
    //!
    //! Provides global profile initialization that loads configuration from
    //! YAML files.
    pub use systemprompt_models::profile::{
        CloudConfig, CloudValidationMode, Profile, ProfileStyle,
    };
    pub use systemprompt_models::profile_bootstrap::{ProfileBootstrap, ProfileBootstrapError};
}

// =============================================================================
// CREDENTIALS BOOTSTRAP
// =============================================================================

#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub mod credentials {
    //! Credential bootstrap system for cloud authentication.
    //!
    //! Provides global credential initialization that loads cloud credentials
    //! based on profile configuration.
    pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};
}

// Root-level re-export for convenience
#[cfg(feature = "cloud")]
#[cfg_attr(docsrs, doc(cfg(feature = "cloud")))]
pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError};

// =============================================================================
// SECRETS
// =============================================================================

// Secrets are now in systemprompt_models - use
// systemprompt_models::SecretsBootstrap directly

// =============================================================================
// PRELUDE
// =============================================================================

mod prelude;

/// Re-export the prelude module for convenient imports.
///
/// ```rust,ignore
/// use systemprompt::prelude::*;
/// ```
pub use crate::prelude::*;
