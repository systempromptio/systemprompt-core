//! Edge cases, boundary tests, and field access tests

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};

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
