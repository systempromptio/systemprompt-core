//! Tests for `CliConfig::resolve` env handling and query methods.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
use systemprompt_cli::env_overrides::EnvOverrides;

fn env_of(vars: &[(&str, &str)]) -> EnvOverrides {
    EnvOverrides::from_vars(vars.iter().copied())
}

#[test]
fn resolve_empty_env_yields_defaults() {
    let cfg = CliConfig::resolve(&env_of(&[]));
    assert_eq!(cfg.output_format, OutputFormat::Table);
    assert_eq!(cfg.verbosity, VerbosityLevel::Normal);
    assert_eq!(cfg.color_mode, ColorMode::Auto);
    assert!(cfg.interactive);
    assert!(cfg.profile_override.is_none());
}

#[test]
fn resolve_output_format_json() {
    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_OUTPUT_FORMAT", "json")]));
    assert_eq!(cfg.output_format, OutputFormat::Json);
}

#[test]
fn resolve_output_format_yaml_case_insensitive() {
    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_OUTPUT_FORMAT", "YAML")]));
    assert_eq!(cfg.output_format, OutputFormat::Yaml);
}

#[test]
fn resolve_output_format_invalid_keeps_default() {
    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_OUTPUT_FORMAT", "xml")]));
    assert_eq!(cfg.output_format, OutputFormat::Table);
}

#[test]
fn resolve_log_level_variants() {
    let cases = [
        ("quiet", VerbosityLevel::Quiet),
        ("normal", VerbosityLevel::Normal),
        ("verbose", VerbosityLevel::Verbose),
        ("DEBUG", VerbosityLevel::Debug),
    ];
    for (value, expected) in cases {
        let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_LOG_LEVEL", value)]));
        assert_eq!(cfg.verbosity, expected, "Failed for {value}");
    }
}

#[test]
fn resolve_log_level_invalid_keeps_default() {
    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_LOG_LEVEL", "loud")]));
    assert_eq!(cfg.verbosity, VerbosityLevel::Normal);
}

#[test]
fn resolve_no_color_disables_color() {
    let cfg = CliConfig::resolve(&env_of(&[("NO_COLOR", "1")]));
    assert_eq!(cfg.color_mode, ColorMode::Never);

    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_NO_COLOR", "1")]));
    assert_eq!(cfg.color_mode, ColorMode::Never);
}

#[test]
fn resolve_non_interactive_disables_interactivity() {
    let cfg = CliConfig::resolve(&env_of(&[("SYSTEMPROMPT_NON_INTERACTIVE", "1")]));
    assert!(!cfg.interactive);
}

#[test]
fn flag_overrides_env_overrides_default() {
    let env = env_of(&[("SYSTEMPROMPT_OUTPUT_FORMAT", "json")]);

    let default_only = CliConfig::resolve(&env_of(&[]));
    assert_eq!(default_only.output_format, OutputFormat::Table);

    let env_only = CliConfig::resolve(&env);
    assert_eq!(env_only.output_format, OutputFormat::Json);

    let flag_over_env = CliConfig::resolve(&env).with_output_format(OutputFormat::Yaml);
    assert_eq!(flag_over_env.output_format, OutputFormat::Yaml);
}

#[test]
fn flag_overrides_env_verbosity() {
    let env = env_of(&[("SYSTEMPROMPT_LOG_LEVEL", "quiet")]);
    let cfg = CliConfig::resolve(&env).with_verbosity(VerbosityLevel::Debug);
    assert_eq!(cfg.verbosity, VerbosityLevel::Debug);
}

#[test]
fn cli_config_new_matches_default() {
    let cfg = CliConfig::new();
    assert_eq!(cfg.output_format, OutputFormat::Table);
    assert_eq!(cfg.verbosity, VerbosityLevel::Normal);
    assert_eq!(cfg.color_mode, ColorMode::Auto);
    assert!(cfg.interactive);
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
fn should_use_color_auto_mode_does_not_panic() {
    let cfg = CliConfig::default().with_color_mode(ColorMode::Auto);
    let _ = cfg.should_use_color();
}

#[test]
fn is_interactive_when_disabled_returns_false() {
    let cfg = CliConfig::default().with_interactive(false);
    assert!(!cfg.is_interactive());
}
