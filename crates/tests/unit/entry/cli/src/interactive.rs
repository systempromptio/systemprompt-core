//! Unit tests for the `interactive` module.
//!
//! All tests force non-interactive mode via `CliConfig::with_interactive(false)`
//! so the dialoguer prompts never fire — every public function in `interactive`
//! has an early-return branch for that case, and those branches are what we
//! cover.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::interactive::{
    confirm_optional, prompt_input, prompt_input_with_default, require_confirmation,
    require_confirmation_default_yes, resolve_required, select_from_list, select_index,
};
use systemprompt_cli::{CliConfig, VerbosityLevel};

fn non_interactive() -> CliConfig {
    CliConfig::default()
        .with_interactive(false)
        .with_verbosity(VerbosityLevel::Quiet)
}

#[test]
fn require_confirmation_skip_is_ok() {
    let cfg = non_interactive();
    require_confirmation("delete everything?", true, &cfg).expect("skip should bypass prompt");
}

#[test]
fn require_confirmation_non_interactive_without_yes_errors() {
    let cfg = non_interactive();
    let err = require_confirmation("delete?", false, &cfg).unwrap_err();
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn require_confirmation_default_yes_skip_is_ok() {
    let cfg = non_interactive();
    require_confirmation_default_yes("proceed?", true, &cfg).expect("skip should bypass prompt");
}

#[test]
fn require_confirmation_default_yes_non_interactive_errors() {
    let cfg = non_interactive();
    let err = require_confirmation_default_yes("proceed?", false, &cfg).unwrap_err();
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
    let err = select_from_list("pick", &items, "thing", &cfg).unwrap_err();
    assert!(err.to_string().contains("No items"));
}

#[test]
fn select_from_list_non_interactive_with_items_errors() {
    let cfg = non_interactive();
    let items = vec!["one".to_string(), "two".to_string()];
    let err = select_from_list("pick", &items, "thing", &cfg).unwrap_err();
    assert!(err.to_string().contains("--thing"));
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn select_index_non_interactive_returns_none() {
    let cfg = non_interactive();
    let items = ["a", "b", "c"];
    let got = select_index("pick", &items, &cfg).expect("call should succeed");
    assert!(got.is_none());
}

#[test]
fn prompt_input_non_interactive_errors() {
    let cfg = non_interactive();
    let err = prompt_input("name?", "name", &cfg).unwrap_err();
    assert!(err.to_string().contains("--name"));
    assert!(err.to_string().contains("non-interactive"));
}

#[test]
fn prompt_input_with_default_non_interactive_returns_default() {
    let cfg = non_interactive();
    let got = prompt_input_with_default("name?", "anon", &cfg).expect("call should succeed");
    assert_eq!(got, "anon");
}

#[test]
fn confirm_optional_non_interactive_returns_default_true() {
    let cfg = non_interactive();
    let got = confirm_optional("ok?", true, &cfg).expect("call should succeed");
    assert!(got);
}

#[test]
fn confirm_optional_non_interactive_returns_default_false() {
    let cfg = non_interactive();
    let got = confirm_optional("ok?", false, &cfg).expect("call should succeed");
    assert!(!got);
}
