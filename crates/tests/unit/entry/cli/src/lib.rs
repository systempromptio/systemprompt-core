//! Unit tests for systemprompt-cli crate
//!
//! Tests cover:
//! - CLI configuration and settings (OutputFormat, VerbosityLevel, ColorMode,
//!   CliConfig)
//! - Builder pattern for CliConfig
//! - Environment variable parsing
//! - Project root discovery
//! - Path handling utilities
//! - Command requirements system
//! - Command result types and builders
//! - CLI parsers for identifiers
//! - Profile utilities

#![allow(clippy::all)]

#[cfg(test)]
mod agents_shared_prompt;
#[cfg(test)]
mod cli_settings;
#[cfg(test)]
mod cloud_deploy_progress;
#[cfg(test)]
mod cloud_init_templates;
#[cfg(test)]
mod cloud_profile_api_keys;
#[cfg(test)]
mod cloud_profile_show_types;
#[cfg(test)]
mod cloud_tenant_docker;
#[cfg(test)]
mod cloud_tenant_validate_ai;
#[cfg(test)]
mod commands;
#[cfg(test)]
mod descriptor;
#[cfg(test)]
mod env_overrides;
#[cfg(test)]
mod environment;
#[cfg(test)]
mod interactive;
#[cfg(test)]
mod paths;
#[cfg(test)]
mod presentation_tables;
#[cfg(test)]
mod shared;
