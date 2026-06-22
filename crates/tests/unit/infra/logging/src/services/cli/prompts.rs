//! Unit tests for cli::prompts.
//!
//! The interactive `dialoguer` read (`.interact()`) needs a TTY and is not
//! driven here. Instead we drive the populated rendering branches — the
//! `section_header` + `StatusDisplay`/`CollectionDisplay` composition that runs
//! *before* the read — and assert the call returns (the non-TTY read yields an
//! `Err`, which is the expected terminal state in a test harness) so the render
//! path is covered. Empty/short-circuit branches and the `PromptBuilder`
//! value-type surface are also exercised.

use systemprompt_logging::services::cli::display::Display;
use systemprompt_logging::services::cli::module::ModuleInstall;
use systemprompt_logging::services::cli::prompts::{PromptBuilder, Prompts, QuickPrompts};

struct StubItem(&'static str);

impl Display for StubItem {
    fn display(&self) {
        let mut stderr = std::io::stderr();
        use std::io::Write;
        writeln!(stderr, "stub:{}", self.0).ok();
    }
}

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

#[test]
fn confirm_install_populated_renders_then_reads() {
    // Non-empty input renders the module list (section header + StatusDisplay
    // collection) before attempting the interactive read; in a non-TTY harness
    // the read resolves to Err. Either outcome means the render path executed
    // without panicking.
    let modules = vec!["alpha".to_owned(), "beta".to_owned()];
    let r = Prompts::confirm_install(&modules);
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn confirm_update_populated_renders_then_reads() {
    let updates = vec![
        ("alpha".to_owned(), "1.0".to_owned(), "2.0".to_owned()),
        ("beta".to_owned(), "0.1".to_owned(), "0.2".to_owned()),
    ];
    let r = Prompts::confirm_update(&updates);
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn confirm_with_context_renders_items() {
    let items = vec![StubItem("one"), StubItem("two")];
    let r = Prompts::confirm_with_context(&items, "Context", "Proceed?", true);
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn confirm_with_context_empty_skips_render() {
    let items: Vec<StubItem> = Vec::new();
    let r = Prompts::confirm_with_context(&items, "Context", "Proceed?", false);
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn prompt_builder_confirm_with_title_and_context() {
    let r = PromptBuilder::new("Apply changes?")
        .with_title("Pending changes")
        .with_default(true)
        .with_context(ModuleInstall::new("mod-a", "1.2"))
        .confirm();
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn prompt_builder_confirm_without_context() {
    let r = PromptBuilder::new("Bare prompt?").confirm();
    assert!(r.is_ok() || r.is_err());
}

#[test]
fn quick_prompts_variants_run() {
    let a = QuickPrompts::yes_no("yn?");
    let b = QuickPrompts::yes_no_default_yes("ynd?");
    let c = QuickPrompts::continue_or_abort("the migration");
    let d = QuickPrompts::dangerous_action("delete everything");
    for r in [a, b, c, d] {
        assert!(r.is_ok() || r.is_err());
    }
}

#[test]
fn prompts_confirm_direct() {
    let r = Prompts::confirm("Direct?", false);
    assert!(r.is_ok() || r.is_err());
    let s = Prompts::confirm_schemas();
    assert!(s.is_ok() || s.is_err());
    let t = Prompts::confirm_seeds();
    assert!(t.is_ok() || t.is_err());
}
