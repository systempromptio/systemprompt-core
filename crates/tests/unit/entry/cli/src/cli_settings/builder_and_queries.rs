//! Tests for CliConfig builder pattern and query methods

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};

// ============================================================================
// CliConfig Builder Tests
// ============================================================================

#[test]
fn test_cli_config_with_output_format_json() {
    let config = CliConfig::default().with_output_format(OutputFormat::Json);
    assert_eq!(config.output_format, OutputFormat::Json);
}

#[test]
fn test_cli_config_with_output_format_yaml() {
    let config = CliConfig::default().with_output_format(OutputFormat::Yaml);
    assert_eq!(config.output_format, OutputFormat::Yaml);
}

#[test]
fn test_cli_config_with_output_format_table() {
    let config = CliConfig::default()
        .with_output_format(OutputFormat::Json)
        .with_output_format(OutputFormat::Table);
    assert_eq!(config.output_format, OutputFormat::Table);
}

#[test]
fn test_cli_config_with_verbosity_quiet() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Quiet);
    assert_eq!(config.verbosity, VerbosityLevel::Quiet);
}

#[test]
fn test_cli_config_with_verbosity_verbose() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Verbose);
    assert_eq!(config.verbosity, VerbosityLevel::Verbose);
}

#[test]
fn test_cli_config_with_verbosity_debug() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Debug);
    assert_eq!(config.verbosity, VerbosityLevel::Debug);
}

#[test]
fn test_cli_config_with_color_mode_always() {
    let config = CliConfig::default().with_color_mode(ColorMode::Always);
    assert_eq!(config.color_mode, ColorMode::Always);
}

#[test]
fn test_cli_config_with_color_mode_never() {
    let config = CliConfig::default().with_color_mode(ColorMode::Never);
    assert_eq!(config.color_mode, ColorMode::Never);
}

#[test]
fn test_cli_config_with_interactive_false() {
    let config = CliConfig::default().with_interactive(false);
    assert!(!config.interactive);
}

#[test]
fn test_cli_config_with_interactive_true() {
    let config = CliConfig::default()
        .with_interactive(false)
        .with_interactive(true);
    assert!(config.interactive);
}

#[test]
fn test_cli_config_builder_chaining() {
    let config = CliConfig::default()
        .with_output_format(OutputFormat::Json)
        .with_verbosity(VerbosityLevel::Debug)
        .with_color_mode(ColorMode::Never)
        .with_interactive(false);

    assert_eq!(config.output_format, OutputFormat::Json);
    assert_eq!(config.verbosity, VerbosityLevel::Debug);
    assert_eq!(config.color_mode, ColorMode::Never);
    assert!(!config.interactive);
}

#[test]
fn test_cli_config_builder_preserves_unmodified_fields() {
    let config = CliConfig::default().with_output_format(OutputFormat::Json);

    assert_eq!(config.verbosity, VerbosityLevel::Normal);
    assert_eq!(config.color_mode, ColorMode::Auto);
    assert!(config.interactive);
}

// ============================================================================
// CliConfig Query Method Tests
// ============================================================================

#[test]
fn test_is_json_output_true_when_json() {
    let config = CliConfig::default().with_output_format(OutputFormat::Json);
    assert!(config.is_json_output());
}

#[test]
fn test_is_json_output_false_when_table() {
    let config = CliConfig::default().with_output_format(OutputFormat::Table);
    assert!(!config.is_json_output());
}

#[test]
fn test_is_json_output_false_when_yaml() {
    let config = CliConfig::default().with_output_format(OutputFormat::Yaml);
    assert!(!config.is_json_output());
}

#[test]
fn test_should_show_verbose_true_when_verbose() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Verbose);
    assert!(config.should_show_verbose());
}

#[test]
fn test_should_show_verbose_true_when_debug() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Debug);
    assert!(config.should_show_verbose());
}

#[test]
fn test_should_show_verbose_false_when_normal() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Normal);
    assert!(!config.should_show_verbose());
}

#[test]
fn test_should_show_verbose_false_when_quiet() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Quiet);
    assert!(!config.should_show_verbose());
}

#[test]
fn test_should_use_color_true_when_always() {
    let config = CliConfig::default().with_color_mode(ColorMode::Always);
    assert!(config.should_use_color());
}

#[test]
fn test_should_use_color_false_when_never() {
    let config = CliConfig::default().with_color_mode(ColorMode::Never);
    assert!(!config.should_use_color());
}

// ============================================================================
// CliConfig Clone and Debug Tests
// ============================================================================

#[test]
fn test_cli_config_clone() {
    let original = CliConfig::default()
        .with_output_format(OutputFormat::Json)
        .with_verbosity(VerbosityLevel::Debug);
    let cloned = original.clone();

    assert_eq!(original.output_format, cloned.output_format);
    assert_eq!(original.verbosity, cloned.verbosity);
    assert_eq!(original.color_mode, cloned.color_mode);
    assert_eq!(original.interactive, cloned.interactive);
}

#[test]
fn test_cli_config_clone_independence() {
    let original = CliConfig::default();
    let mut cloned = original.clone();
    cloned = cloned.with_output_format(OutputFormat::Yaml);

    assert_eq!(original.output_format, OutputFormat::Table);
    assert_eq!(cloned.output_format, OutputFormat::Yaml);
}

#[test]
fn test_cli_config_debug() {
    let config = CliConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("CliConfig"));
    assert!(debug_str.contains("output_format"));
    assert!(debug_str.contains("verbosity"));
}
