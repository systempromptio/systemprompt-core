//! Unit tests for CLI settings module
//!
//! Tests cover:
//! - OutputFormat enum variants and default behavior
//! - VerbosityLevel enum variants and ordering
//! - ColorMode enum variants
//! - CliConfig builder pattern and fluent API
//! - CliConfig default values
//! - Boolean query methods

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

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
fn test_output_format_not_equal() {
    assert_ne!(OutputFormat::Table, OutputFormat::Json);
    assert_ne!(OutputFormat::Json, OutputFormat::Yaml);
    assert_ne!(OutputFormat::Table, OutputFormat::Yaml);
}

#[test]
fn test_output_format_copy() {
    let format = OutputFormat::Json;
    let copied = format;
    assert_eq!(format, copied);
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
fn test_verbosity_level_copy() {
    let level = VerbosityLevel::Verbose;
    let copied = level;
    assert_eq!(level, copied);
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
fn test_color_mode_not_equal() {
    assert_ne!(ColorMode::Auto, ColorMode::Always);
    assert_ne!(ColorMode::Always, ColorMode::Never);
    assert_ne!(ColorMode::Auto, ColorMode::Never);
}

#[test]
fn test_color_mode_copy() {
    let mode = ColorMode::Always;
    let copied = mode;
    assert_eq!(mode, copied);
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

    // These should remain at defaults
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
// CliConfig Clone Tests
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

    // Original should be unchanged
    assert_eq!(original.output_format, OutputFormat::Table);
    assert_eq!(cloned.output_format, OutputFormat::Yaml);
}

// ============================================================================
// CliConfig Debug Tests
// ============================================================================

#[test]
fn test_cli_config_debug() {
    let config = CliConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("CliConfig"));
    assert!(debug_str.contains("output_format"));
    assert!(debug_str.contains("verbosity"));
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_multiple_format_changes() {
    let config = CliConfig::default()
        .with_output_format(OutputFormat::Json)
        .with_output_format(OutputFormat::Yaml)
        .with_output_format(OutputFormat::Table)
        .with_output_format(OutputFormat::Json);

    assert_eq!(config.output_format, OutputFormat::Json);
}

#[test]
fn test_multiple_verbosity_changes() {
    let config = CliConfig::default()
        .with_verbosity(VerbosityLevel::Debug)
        .with_verbosity(VerbosityLevel::Quiet)
        .with_verbosity(VerbosityLevel::Verbose);

    assert_eq!(config.verbosity, VerbosityLevel::Verbose);
}

#[test]
fn test_all_verbosity_levels_with_should_show_verbose() {
    let levels_and_expected = [
        (VerbosityLevel::Quiet, false),
        (VerbosityLevel::Normal, false),
        (VerbosityLevel::Verbose, true),
        (VerbosityLevel::Debug, true),
    ];

    for (level, expected) in levels_and_expected {
        let config = CliConfig::default().with_verbosity(level);
        assert_eq!(
            config.should_show_verbose(),
            expected,
            "Failed for {:?}",
            level
        );
    }
}

#[test]
fn test_all_color_modes_with_should_use_color() {
    let modes_and_expected = [
        (ColorMode::Always, true),
        (ColorMode::Never, false),
        // ColorMode::Auto depends on terminal, skip in unit tests
    ];

    for (mode, expected) in modes_and_expected {
        let config = CliConfig::default().with_color_mode(mode);
        assert_eq!(
            config.should_use_color(),
            expected,
            "Failed for {:?}",
            mode
        );
    }
}

#[test]
fn test_all_output_formats_with_is_json() {
    let formats_and_expected = [
        (OutputFormat::Table, false),
        (OutputFormat::Json, true),
        (OutputFormat::Yaml, false),
    ];

    for (format, expected) in formats_and_expected {
        let config = CliConfig::default().with_output_format(format);
        assert_eq!(
            config.is_json_output(),
            expected,
            "Failed for {:?}",
            format
        );
    }
}

// ============================================================================
// Field Access Tests
// ============================================================================

#[test]
fn test_direct_field_access_output_format() {
    let config = CliConfig::default().with_output_format(OutputFormat::Yaml);
    assert_eq!(config.output_format, OutputFormat::Yaml);
}

#[test]
fn test_direct_field_access_verbosity() {
    let config = CliConfig::default().with_verbosity(VerbosityLevel::Debug);
    assert_eq!(config.verbosity, VerbosityLevel::Debug);
}

#[test]
fn test_direct_field_access_color_mode() {
    let config = CliConfig::default().with_color_mode(ColorMode::Never);
    assert_eq!(config.color_mode, ColorMode::Never);
}

#[test]
fn test_direct_field_access_interactive() {
    let config = CliConfig::default().with_interactive(false);
    assert!(!config.interactive);
}
