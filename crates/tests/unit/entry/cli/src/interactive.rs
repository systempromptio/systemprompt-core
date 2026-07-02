//! Unit tests for the `interactive` module.
//!
//! The flag-bridging helpers are tested in non-interactive mode via
//! `CliConfig::with_interactive(false)` so the dialoguer prompts never fire —
//! every public function in `interactive` has an early-return branch for that
//! case. Prompting behaviour itself is covered through `ScriptedPrompter`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::interactive::{
    Prompter, ScriptedPrompter, confirm_optional, prompt_input, prompt_input_with_default,
    require_confirmation, require_confirmation_default_yes, resolve_required, select_from_list,
    select_index,
};
use systemprompt_cli::{CliConfig, VerbosityLevel};

fn non_interactive() -> CliConfig {
    CliConfig::default()
        .with_interactive(false)
        .with_verbosity(VerbosityLevel::Quiet)
}

fn interactive() -> CliConfig {
    CliConfig::default()
        .with_interactive(true)
        .with_verbosity(VerbosityLevel::Quiet)
}

fn no_answers() -> ScriptedPrompter {
    ScriptedPrompter::default()
}

#[test]
fn require_confirmation_skip_is_ok() {
    let cfg = non_interactive();
    require_confirmation(&no_answers(), "delete everything?", true, &cfg)
        .expect("skip should bypass prompt");
}

#[test]
fn require_confirmation_non_interactive_without_yes_errors() {
    let cfg = non_interactive();
    let err = require_confirmation(&no_answers(), "delete?", false, &cfg).unwrap_err();
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn require_confirmation_default_yes_skip_is_ok() {
    let cfg = non_interactive();
    require_confirmation_default_yes(&no_answers(), "proceed?", true, &cfg)
        .expect("skip should bypass prompt");
}

#[test]
fn require_confirmation_default_yes_non_interactive_errors() {
    let cfg = non_interactive();
    let err = require_confirmation_default_yes(&no_answers(), "proceed?", false, &cfg).unwrap_err();
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn resolve_required_returns_provided_value() {
    let cfg = non_interactive();
    let v = resolve_required(Some(42_u32), "count", &cfg, || Ok(0)).expect("value passes through");
    assert_eq!(v, 42);
}

#[test]
fn resolve_required_non_interactive_without_value_errors() {
    let cfg = non_interactive();
    let result: anyhow::Result<u32> = resolve_required(None, "count", &cfg, || Ok(0));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("--count"));
    assert!(err.contains("non-interactive"));
}

#[test]
fn select_from_list_empty_items_errors() {
    let cfg = non_interactive();
    let items: Vec<String> = Vec::new();
    let err = select_from_list(&no_answers(), "pick", &items, "thing", &cfg).unwrap_err();
    assert!(err.to_string().contains("No items"));
}

#[test]
fn select_from_list_non_interactive_with_items_errors() {
    let cfg = non_interactive();
    let items = vec!["one".to_string(), "two".to_string()];
    let err = select_from_list(&no_answers(), "pick", &items, "thing", &cfg).unwrap_err();
    assert!(err.to_string().contains("--thing"));
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn select_index_non_interactive_returns_none() {
    let cfg = non_interactive();
    let items = ["a", "b", "c"];
    let got = select_index(&no_answers(), "pick", &items, &cfg).expect("call should succeed");
    assert!(got.is_none());
}

#[test]
fn prompt_input_non_interactive_errors() {
    let cfg = non_interactive();
    let err = prompt_input(&no_answers(), "name?", "name", &cfg).unwrap_err();
    assert!(err.to_string().contains("--name"));
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn prompt_input_with_default_non_interactive_returns_default() {
    let cfg = non_interactive();
    let got = prompt_input_with_default(&no_answers(), "name?", "anon", &cfg)
        .expect("call should succeed");
    assert_eq!(got, "anon");
}

#[test]
fn confirm_optional_non_interactive_returns_default_true() {
    let cfg = non_interactive();
    let got = confirm_optional(&no_answers(), "ok?", true, &cfg).expect("call should succeed");
    assert!(got);
}

#[test]
fn confirm_optional_non_interactive_returns_default_false() {
    let cfg = non_interactive();
    let got = confirm_optional(&no_answers(), "ok?", false, &cfg).expect("call should succeed");
    assert!(!got);
}

#[test]
fn scripted_prompter_consumes_answers_in_order() {
    let prompter = ScriptedPrompter::new(["first", "second"]);
    assert_eq!(prompter.input("a?").expect("first answer"), "first");
    assert_eq!(prompter.input("b?").expect("second answer"), "second");
}

#[test]
fn scripted_prompter_exhausted_errors_with_prompt() {
    let prompter = ScriptedPrompter::new(std::iter::empty::<String>());
    let err = prompter.input("name?").unwrap_err();
    assert!(err.to_string().contains("exhausted"));
    assert!(err.to_string().contains("name?"));
}

#[test]
fn scripted_prompter_confirm_parses_affirmatives() {
    let prompter = ScriptedPrompter::new(["y", "yes", "TRUE", "no", "anything"]);
    assert!(prompter.confirm("ok?", false).expect("y"));
    assert!(prompter.confirm("ok?", false).expect("yes"));
    assert!(prompter.confirm("ok?", false).expect("true"));
    assert!(!prompter.confirm("ok?", true).expect("no"));
    assert!(!prompter.confirm("ok?", true).expect("anything"));
}

#[test]
fn scripted_prompter_input_with_default_empty_answer_uses_default() {
    let prompter = ScriptedPrompter::new([""]);
    let got = prompter
        .input_with_default("name?", "anon")
        .expect("call should succeed");
    assert_eq!(got, "anon");
}

#[test]
fn scripted_prompter_input_with_default_non_empty_answer_wins() {
    let prompter = ScriptedPrompter::new(["given"]);
    let got = prompter
        .input_with_default("name?", "anon")
        .expect("call should succeed");
    assert_eq!(got, "given");
}

#[test]
fn scripted_prompter_select_parses_index() {
    let prompter = ScriptedPrompter::new(["1"]);
    let items = vec!["a".to_string(), "b".to_string()];
    let got = prompter
        .select("pick", &items)
        .expect("call should succeed");
    assert_eq!(got, 1);
}

#[test]
fn scripted_prompter_select_out_of_range_errors() {
    let prompter = ScriptedPrompter::new(["2"]);
    let items = vec!["a".to_string(), "b".to_string()];
    let err = prompter.select("pick", &items).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn scripted_prompter_select_non_numeric_errors() {
    let prompter = ScriptedPrompter::new(["b"]);
    let items = vec!["a".to_string(), "b".to_string()];
    let err = prompter.select("pick", &items).unwrap_err();
    assert!(err.to_string().contains("not an index"));
}

#[test]
fn scripted_prompter_password_returns_next_answer() {
    let prompter = ScriptedPrompter::new(["s3cret"]);
    let got = prompter.password("password?").expect("call should succeed");
    assert_eq!(got, "s3cret");
}

#[test]
fn scripted_prompter_answers_shared_across_methods() {
    let prompter = ScriptedPrompter::new(["yes", "ed", "0"]);
    let items = vec!["only".to_string()];
    assert!(prompter.confirm("ok?", false).expect("confirm"));
    assert_eq!(prompter.input("name?").expect("input"), "ed");
    assert_eq!(prompter.select("pick", &items).expect("select"), 0);
    assert!(prompter.input("extra?").is_err());
}

#[test]
fn require_confirmation_flag_alone_does_not_enable_prompting_without_tty() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["y"]);
    let err = require_confirmation(&prompter, "delete?", false, &cfg).unwrap_err();
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn require_confirmation_default_yes_flag_alone_does_not_prompt_without_tty() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["yes"]);
    let err = require_confirmation_default_yes(&prompter, "proceed?", false, &cfg).unwrap_err();
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn select_from_list_without_tty_requires_flag() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["1"]);
    let items = vec!["one".to_string(), "two".to_string()];
    let err = select_from_list(&prompter, "pick", &items, "thing", &cfg).unwrap_err();
    assert!(err.to_string().contains("--thing"));
}

#[test]
fn select_index_without_tty_returns_none() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["2"]);
    let items = ["a", "b", "c"];
    let got = select_index(&prompter, "pick", &items, &cfg).expect("selected");
    assert_eq!(got, None);
}

#[test]
fn prompt_input_without_tty_requires_flag() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["ed"]);
    let err = prompt_input(&prompter, "name?", "name", &cfg).unwrap_err();
    assert!(err.to_string().contains("--name"));
}

#[test]
fn prompt_input_with_default_without_tty_returns_default() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["given"]);
    let got = prompt_input_with_default(&prompter, "name?", "anon", &cfg).expect("answered");
    assert_eq!(got, "anon");
}

#[test]
fn confirm_optional_without_tty_returns_default() {
    let cfg = interactive();
    let prompter = ScriptedPrompter::new(["yes"]);
    assert!(!confirm_optional(&prompter, "ok?", false, &cfg).expect("answered"));
}
