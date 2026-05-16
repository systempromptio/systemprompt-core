//! MCP tool-schema handling.
//!
//! Loads JSON schemas for server tools and validates tool inputs and
//! outputs against them.

pub mod loader;
pub mod validator;

pub use loader::SchemaLoader;
pub use validator::{SchemaValidationMode, SchemaValidationReport, SchemaValidator};
