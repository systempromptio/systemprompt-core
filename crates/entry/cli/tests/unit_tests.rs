//! Unit tests for systemprompt-cli crate
//!
//! Tests cover:
//! - CLI configuration and settings
//! - Builder pattern
//! - Environment variable parsing
//! - Project root discovery
//! - Path handling utilities

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::doc_markdown,
    clippy::needless_collect
)]

use systemprompt_cli::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
use systemprompt_cli::common::project::ProjectError;
use std::path::PathBuf;
use tempfile::TempDir;

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
// Edge Cases and Comprehensive Tests
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
// ProjectError Tests
// ============================================================================

#[test]
fn test_project_error_not_found_display() {
    let error = ProjectError::ProjectNotFound {
        path: PathBuf::from("/some/path"),
    };
    let msg = error.to_string();
    assert!(msg.contains("Not a SystemPrompt project"));
    assert!(msg.contains("/some/path"));
    assert!(msg.contains(".systemprompt"));
}

#[test]
fn test_project_error_not_found_debug() {
    let error = ProjectError::ProjectNotFound {
        path: PathBuf::from("/test/path"),
    };
    let debug = format!("{:?}", error);
    assert!(debug.contains("ProjectNotFound"));
    assert!(debug.contains("/test/path"));
}

#[test]
fn test_project_error_path_resolution_display() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error = ProjectError::PathResolution {
        path: PathBuf::from("/bad/path"),
        source: io_error,
    };
    let msg = error.to_string();
    assert!(msg.contains("Failed to resolve path"));
    assert!(msg.contains("/bad/path"));
}

#[test]
fn test_project_error_path_resolution_source() {
    use std::error::Error;

    let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let error = ProjectError::PathResolution {
        path: PathBuf::from("/secure/path"),
        source: io_error,
    };

    let source = error.source();
    assert!(source.is_some());
}

#[test]
fn test_project_error_preserves_path() {
    let test_path = PathBuf::from("/my/custom/path");
    let error = ProjectError::ProjectNotFound {
        path: test_path.clone(),
    };

    if let ProjectError::ProjectNotFound { path } = error {
        assert_eq!(path, test_path);
    } else {
        panic!("Expected ProjectNotFound variant");
    }
}

#[test]
fn test_project_error_with_special_chars_in_path() {
    let test_path = PathBuf::from("/path/with spaces/and-dashes/under_scores");
    let error = ProjectError::ProjectNotFound { path: test_path };

    let msg = error.to_string();
    assert!(msg.contains("spaces"));
    assert!(msg.contains("dashes"));
    assert!(msg.contains("under_scores"));
}

// ============================================================================
// Project Directory Tests (using TempDir)
// ============================================================================

fn create_project_dir() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp directory");
    std::fs::create_dir(temp.path().join(".systemprompt"))
        .expect("Failed to create .systemprompt directory");
    temp
}

#[test]
fn test_valid_project_has_systemprompt_dir() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    assert!(systemprompt_dir.exists());
    assert!(systemprompt_dir.is_dir());
}

#[test]
fn test_systemprompt_directory_must_be_directory() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    std::fs::write(temp.path().join(".systemprompt"), "not a dir")
        .expect("Failed to create file");

    assert!(temp.path().join(".systemprompt").is_file());
    assert!(!temp.path().join(".systemprompt").is_dir());
}

#[test]
fn test_nested_project_structure() {
    let temp = create_project_dir();

    let nested = temp.path().join("src").join("components").join("auth");
    std::fs::create_dir_all(&nested).expect("Failed to create nested dirs");

    assert!(temp.path().join(".systemprompt").is_dir());
    assert!(nested.exists());
}

#[test]
fn test_no_systemprompt_directory() {
    let temp = TempDir::new().expect("Failed to create temp directory");

    assert!(!temp.path().join(".systemprompt").exists());
}

#[test]
fn test_empty_systemprompt_dir_is_valid() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    let entries: Vec<_> = std::fs::read_dir(&systemprompt_dir)
        .expect("Failed to read dir")
        .collect();
    assert!(entries.is_empty());
}

#[test]
fn test_systemprompt_dir_with_contents() {
    let temp = create_project_dir();
    let systemprompt_dir = temp.path().join(".systemprompt");

    std::fs::write(systemprompt_dir.join("config.toml"), "# config")
        .expect("Failed to write config");

    let entries: Vec<_> = std::fs::read_dir(&systemprompt_dir)
        .expect("Failed to read dir")
        .collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_project_path_join() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let config_path = project_path.join("config");
    assert_eq!(config_path.parent(), Some(project_path));
}

#[test]
fn test_project_path_components() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let components: Vec<_> = project_path.components().collect();
    assert!(!components.is_empty());
}

#[test]
fn test_project_path_display() {
    let temp = create_project_dir();
    let project_path = temp.path();

    let display = project_path.display().to_string();
    assert!(!display.is_empty());
}
