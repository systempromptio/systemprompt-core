//! Tests for OutputFormat, VerbosityLevel, ColorMode enums and CliConfig
//! defaults

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};

// ============================================================================
// OutputFormat Tests
// ============================================================================

#[test]
fn test_output_format_table_variant() {
    let format = OutputFormat::Table;
    assert_eq!(format, OutputFormat::Table);
}

#[test]
fn test_output_format_json_variant() {
    let format = OutputFormat::Json;
    assert_eq!(format, OutputFormat::Json);
}

#[test]
fn test_output_format_yaml_variant() {
    let format = OutputFormat::Yaml;
    assert_eq!(format, OutputFormat::Yaml);
}

#[test]
fn test_output_format_debug() {
    let format = OutputFormat::Table;
    let debug_str = format!("{:?}", format);
    assert!(debug_str.contains("Table"));
}

// ============================================================================
// VerbosityLevel Tests
// ============================================================================

#[test]
fn test_verbosity_level_quiet_variant() {
    let level = VerbosityLevel::Quiet;
    assert_eq!(level, VerbosityLevel::Quiet);
}

#[test]
fn test_verbosity_level_normal_variant() {
    let level = VerbosityLevel::Normal;
    assert_eq!(level, VerbosityLevel::Normal);
}

#[test]
fn test_verbosity_level_verbose_variant() {
    let level = VerbosityLevel::Verbose;
    assert_eq!(level, VerbosityLevel::Verbose);
}

#[test]
fn test_verbosity_level_debug_variant() {
    let level = VerbosityLevel::Debug;
    assert_eq!(level, VerbosityLevel::Debug);
}

#[test]
fn test_verbosity_level_ordering() {
    assert!(VerbosityLevel::Quiet < VerbosityLevel::Normal);
    assert!(VerbosityLevel::Normal < VerbosityLevel::Verbose);
    assert!(VerbosityLevel::Verbose < VerbosityLevel::Debug);
}

#[test]
fn test_verbosity_level_ordering_transitive() {
    assert!(VerbosityLevel::Quiet < VerbosityLevel::Debug);
    assert!(VerbosityLevel::Normal < VerbosityLevel::Debug);
    assert!(VerbosityLevel::Quiet < VerbosityLevel::Verbose);
}

#[test]
fn test_verbosity_level_debug_format() {
    let level = VerbosityLevel::Debug;
    let debug_str = format!("{:?}", level);
    assert!(debug_str.contains("Debug"));
}

// ============================================================================
// ColorMode Tests
// ============================================================================

#[test]
fn test_color_mode_auto_variant() {
    let mode = ColorMode::Auto;
    assert_eq!(mode, ColorMode::Auto);
}

#[test]
fn test_color_mode_always_variant() {
    let mode = ColorMode::Always;
    assert_eq!(mode, ColorMode::Always);
}

#[test]
fn test_color_mode_never_variant() {
    let mode = ColorMode::Never;
    assert_eq!(mode, ColorMode::Never);
}

#[test]
fn test_color_mode_debug() {
    let mode = ColorMode::Never;
    let debug_str = format!("{:?}", mode);
    assert!(debug_str.contains("Never"));
}

// ============================================================================
// CliConfig Default Tests
// ============================================================================

#[test]
fn test_cli_config_default_output_format() {
    let config = CliConfig::default();
    assert_eq!(config.output_format, OutputFormat::Table);
}

#[test]
fn test_cli_config_default_verbosity() {
    let config = CliConfig::default();
    assert_eq!(config.verbosity, VerbosityLevel::Normal);
}

#[test]
fn test_cli_config_default_color_mode() {
    let config = CliConfig::default();
    assert_eq!(config.color_mode, ColorMode::Auto);
}

#[test]
fn test_cli_config_default_interactive() {
    let config = CliConfig::default();
    assert!(config.interactive);
}
