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
mod cli_settings;
#[cfg(test)]
mod descriptor;
#[cfg(test)]
mod shared;
