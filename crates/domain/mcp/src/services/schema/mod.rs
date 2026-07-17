//! MCP tool-schema handling.
//!
//! Loads JSON schemas for server tools and validates tool inputs and
//! outputs against them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod loader;
pub mod validator;

pub use loader::SchemaLoader;
pub use validator::{SchemaValidationMode, SchemaValidationReport, SchemaValidator};
