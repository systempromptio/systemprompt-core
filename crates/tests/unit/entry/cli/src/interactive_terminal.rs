//! Terminal-side tests for the `interactive` flag-bridging helpers.
//!
//! The sibling `interactive` module covers the non-interactive early returns.
//! `CliConfig::with_assume_terminal(true)` is what makes `is_interactive()`
//! true without a real TTY, so these drive the prompting branches through a
//! `ScriptedPrompter`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::interactive::{
    ScriptedPrompter, confirm_optional, prompt_input, prompt_input_with_default,
    require_confirmation, require_confirmation_default_yes, resolve_required, select_from_list,
    select_index,
};
use systemprompt_cli::{CliConfig, VerbosityLevel};

fn terminal() -> CliConfig {
    CliConfig::default()
        .with_interactive(true)
        .with_assume_terminal(true)
        .with_verbosity(VerbosityLevel::Quiet)
}

#[test]
fn confirmed_prompt_allows_the_operation() {
    let prompter = ScriptedPrompter::new(["yes"]);
    require_confirmation(&prompter, "Delete everything?", false, &terminal()).unwrap();
}

#[test]
fn declined_prompt_cancels_the_operation() {
    let prompter = ScriptedPrompter::new(["no"]);
    let err =
        require_confirmation(&prompter, "Delete everything?", false, &terminal()).unwrap_err();
    assert_eq!(err.to_string(), "Operation cancelled");
}

#[test]
fn default_yes_variant_also_honours_the_answer() {
    let prompter = ScriptedPrompter::new(["y"]);
    require_confirmation_default_yes(&prompter, "Proceed?", false, &terminal()).unwrap();

    let prompter = ScriptedPrompter::new(["n"]);
    let err =
        require_confirmation_default_yes(&prompter, "Proceed?", false, &terminal()).unwrap_err();
    assert_eq!(err.to_string(), "Operation cancelled");
}

#[test]
fn resolve_required_falls_back_to_the_prompt_when_the_flag_is_absent() {
    let resolved = resolve_required(
        None,
        "name",
        &terminal(),
        || Ok("prompted-value".to_owned()),
    )
    .unwrap();
    assert_eq!(resolved, "prompted-value");
}

#[test]
fn resolve_required_surfaces_a_failing_prompt() {
    let err = resolve_required(None::<String>, "name", &terminal(), || {
        Err(anyhow::anyhow!("prompt exploded"))
    })
    .unwrap_err();
    assert_eq!(err.to_string(), "prompt exploded");
}

#[test]
fn select_from_list_returns_the_chosen_item() {
    let prompter = ScriptedPrompter::new(["2"]);
    let items = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];

    let chosen = select_from_list(&prompter, "Pick one", &items, "name", &terminal()).unwrap();
    assert_eq!(chosen, "gamma");
}

#[test]
fn select_from_list_rejects_an_out_of_range_answer() {
    let prompter = ScriptedPrompter::new(["9"]);
    let items = vec!["alpha".to_owned()];

    let err = select_from_list(&prompter, "Pick one", &items, "name", &terminal()).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn select_index_returns_the_chosen_position() {
    let prompter = ScriptedPrompter::new(["1"]);
    let chosen = select_index(&prompter, "Pick one", &["first", "second"], &terminal()).unwrap();
    assert_eq!(chosen, Some(1));
}

#[test]
fn prompt_input_returns_the_typed_value() {
    let prompter = ScriptedPrompter::new(["typed"]);
    let value = prompt_input(&prompter, "Name?", "name", &terminal()).unwrap();
    assert_eq!(value, "typed");
}

#[test]
fn prompt_input_with_default_prefers_the_typed_value_and_falls_back_when_empty() {
    let prompter = ScriptedPrompter::new(["typed"]);
    let value = prompt_input_with_default(&prompter, "Name?", "fallback", &terminal()).unwrap();
    assert_eq!(value, "typed");

    let prompter = ScriptedPrompter::new([""]);
    let value = prompt_input_with_default(&prompter, "Name?", "fallback", &terminal()).unwrap();
    assert_eq!(value, "fallback");
}

#[test]
fn confirm_optional_returns_the_answer_rather_than_the_default() {
    let prompter = ScriptedPrompter::new(["yes"]);
    assert!(confirm_optional(&prompter, "Enable?", false, &terminal()).unwrap());

    let prompter = ScriptedPrompter::new(["no"]);
    assert!(!confirm_optional(&prompter, "Enable?", true, &terminal()).unwrap());
}
