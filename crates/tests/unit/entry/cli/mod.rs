//! Unit tests for systemprompt-cli crate
//!
//! Tests cover:
//! - CLI configuration and settings (OutputFormat, VerbosityLevel, ColorMode, CliConfig)
//! - Builder pattern for CliConfig
//! - Environment variable parsing
//! - Project root discovery
//! - Path handling utilities

#![allow(clippy::all)]

mod cli_settings;
mod common;
