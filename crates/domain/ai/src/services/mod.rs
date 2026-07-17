//! Service orchestration layer for the AI domain crate.
//!
//! Contains the top-level [`crate::AiService`] and supporting modules for
//! provider drivers, tool dispatch, structured-output validation, schema
//! transformation, image storage, and config validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config;
pub mod core;
pub mod gateway;
pub mod providers;
pub mod schema;
pub mod storage;
pub mod structured_output;
pub mod tooled;
pub mod tools;

pub use storage::{ImageStorage, StorageConfig};
pub use tools::{NoopToolProvider, ToolDiscovery};
