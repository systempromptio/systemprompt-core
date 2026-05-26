//! Tests for global config storage and `CliConfig::new()` env-var path.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cli_settings::{
    CliConfig, ColorMode, OutputFormat, VerbosityLevel, get_global_config, set_global_config,
};

#[test]
fn global_config_round_trip() {
    let custom = CliConfig::default()
        .with_output_format(OutputFormat::Yaml)
        .with_verbosity(VerbosityLevel::Debug)
        .with_color_mode(ColorMode::Always)
        .with_interactive(false);

    set_global_config(custom.clone());
    let read_back = get_global_config();

    assert_eq!(read_back.output_format, OutputFormat::Yaml);
    assert_eq!(read_back.verbosity, VerbosityLevel::Debug);
    assert_eq!(read_back.color_mode, ColorMode::Always);
    assert!(!read_back.interactive);
}

#[test]
fn global_config_default_clone() {
    set_global_config(CliConfig::default());
    let read_back = get_global_config();
    assert_eq!(read_back.output_format, OutputFormat::Table);
    assert_eq!(read_back.verbosity, VerbosityLevel::Normal);
    assert_eq!(read_back.color_mode, ColorMode::Auto);
    assert!(read_back.interactive);
}

#[test]
fn cli_config_new_invokes_env_branch() {
    let cfg = CliConfig::new();
    let _ = cfg.output_format;
    let _ = cfg.verbosity;
    let _ = cfg.color_mode;
}

#[test]
fn output_format_accessor_table() {
    let cfg = CliConfig::default().with_output_format(OutputFormat::Table);
    assert_eq!(cfg.output_format(), OutputFormat::Table);
}

#[test]
fn output_format_accessor_json() {
    let cfg = CliConfig::default().with_output_format(OutputFormat::Json);
    assert_eq!(cfg.output_format(), OutputFormat::Json);
}

#[test]
fn output_format_accessor_yaml() {
    let cfg = CliConfig::default().with_output_format(OutputFormat::Yaml);
    assert_eq!(cfg.output_format(), OutputFormat::Yaml);
}

#[test]
fn verbosity_as_tracing_filter_levels() {
    assert_eq!(VerbosityLevel::Quiet.as_tracing_filter(), Some("error"));
    assert_eq!(VerbosityLevel::Normal.as_tracing_filter(), None);
    assert_eq!(VerbosityLevel::Verbose.as_tracing_filter(), Some("debug"));
    assert_eq!(VerbosityLevel::Debug.as_tracing_filter(), Some("trace"));
}

#[test]
fn with_profile_override_some() {
    let cfg = CliConfig::default().with_profile_override(Some("prod".to_string()));
    assert_eq!(cfg.profile_override.as_deref(), Some("prod"));
}

#[test]
fn with_profile_override_none() {
    let cfg = CliConfig::default()
        .with_profile_override(Some("dev".to_string()))
        .with_profile_override(None);
    assert!(cfg.profile_override.is_none());
}

#[test]
fn verbosity_ordering() {
    assert!(VerbosityLevel::Quiet < VerbosityLevel::Normal);
    assert!(VerbosityLevel::Normal < VerbosityLevel::Verbose);
    assert!(VerbosityLevel::Verbose < VerbosityLevel::Debug);
}

#[test]
fn should_use_color_auto_mode_does_not_panic() {
    let cfg = CliConfig::default().with_color_mode(ColorMode::Auto);
    let _ = cfg.should_use_color();
}

#[test]
fn is_interactive_when_disabled_returns_false() {
    let cfg = CliConfig::default().with_interactive(false);
    assert!(!cfg.is_interactive());
}
