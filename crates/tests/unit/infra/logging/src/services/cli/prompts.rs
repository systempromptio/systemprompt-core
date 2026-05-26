//! Unit tests for cli::prompts.
//!
//! Interactive `dialoguer` flows are out of scope. We exercise the empty/
//! short-circuit branches and the `PromptBuilder` value-type surface.

use systemprompt_logging::services::cli::prompts::{PromptBuilder, Prompts};
use systemprompt_logging::services::cli::module::ModuleInstall;

#[test]
fn confirm_install_empty_returns_ok_false() {
    let r = Prompts::confirm_install(&[]).unwrap();
    assert!(!r);
}

#[test]
fn confirm_update_empty_returns_ok_false() {
    let r = Prompts::confirm_update(&[]).unwrap();
    assert!(!r);
}

#[test]
fn prompt_builder_records_message() {
    let b = PromptBuilder::new("hi");
    let dbg = format!("{:?}", b);
    assert!(dbg.contains("hi"));
}

#[test]
fn prompt_builder_with_title() {
    let b = PromptBuilder::new("ok?").with_title("Question");
    let dbg = format!("{:?}", b);
    assert!(dbg.contains("Question"));
}

#[test]
fn prompt_builder_with_default_flips_field() {
    let b = PromptBuilder::new("ok?").with_default(true);
    let dbg = format!("{:?}", b);
    assert!(dbg.contains("default: true"));
}

#[test]
fn prompt_builder_with_context_accumulates() {
    let b = PromptBuilder::new("install?")
        .with_context(ModuleInstall::new("a", "1.0"))
        .with_context(ModuleInstall::new("b", "2.0"));
    let dbg = format!("{:?}", b);
    assert!(dbg.contains("2 items"));
}
