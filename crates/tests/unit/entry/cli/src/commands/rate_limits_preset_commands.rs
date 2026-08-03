//! Tests for the `admin config rate-limits preset` command paths.
//!
//! Drives the public `rate_limits::execute` dispatcher so the list, show, and
//! apply arms are exercised end to end, including the unknown-preset rejection
//! that guards every arm.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::rate_limits::{
    PresetApplyArgs, PresetCommands, PresetShowArgs, RateLimitsCommands, execute,
};
use systemprompt_cli::{CliConfig, OutputFormat, ScriptedPrompter};

fn config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

#[test]
fn preset_list_renders_the_builtin_catalogue() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    execute(
        RateLimitsCommands::Preset(PresetCommands::List),
        &prompter,
        &config(),
    )
    .unwrap();
}

#[test]
fn preset_show_renders_each_builtin() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    for name in ["development", "production", "high-traffic"] {
        execute(
            RateLimitsCommands::Preset(PresetCommands::Show(PresetShowArgs {
                name: name.to_owned(),
            })),
            &prompter,
            &config(),
        )
        .unwrap();
    }
}

#[test]
fn preset_show_rejects_an_unknown_name() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = execute(
        RateLimitsCommands::Preset(PresetCommands::Show(PresetShowArgs {
            name: "nonesuch".to_owned(),
        })),
        &prompter,
        &config(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("Unknown preset: nonesuch"));
    assert!(err.to_string().contains("development"));
}

#[test]
fn preset_apply_rejects_an_unknown_name_before_touching_the_profile() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = execute(
        RateLimitsCommands::Preset(PresetCommands::Apply(PresetApplyArgs {
            name: "nonesuch".to_owned(),
            yes: true,
        })),
        &prompter,
        &config(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("Unknown preset: nonesuch"));
}

#[test]
fn preset_apply_declined_at_the_confirmation_prompt_does_not_proceed() {
    let prompter = ScriptedPrompter::new(["no"]);
    let err = execute(
        RateLimitsCommands::Preset(PresetCommands::Apply(PresetApplyArgs {
            name: "production".to_owned(),
            yes: false,
        })),
        &prompter,
        &CliConfig::new().with_interactive(true),
    )
    .unwrap_err();

    assert!(!err.to_string().is_empty());
}
