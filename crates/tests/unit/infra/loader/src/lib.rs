//! Unit tests for systemprompt-loader crate
//!
//! Tests cover:
//! - SecretsLoader path resolution and file loading
//! - IncludeResolver string resolution and YAML file loading
//! - ModuleLoader YAML parsing and category scanning
//! - ProfileLoader file loading and validation
//! - ConfigLoader and EnhancedConfigLoader configuration merging

#![allow(clippy::all)]

mod include_resolver;
mod module_loader;
mod profile_loader;
mod secrets_loader;
mod services_loader;
