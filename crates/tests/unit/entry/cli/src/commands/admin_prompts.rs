//! Tests for the `admin agents` interactive prompts, driven through
//! `ScriptedPrompter` without touching the filesystem, database, or network.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::admin::agents::create::{
    prompt_name, prompt_port, validate_name_input, validate_port_input,
};
use systemprompt_cli::admin::agents::logs_disk::select_agent_from_log_files;
use systemprompt_cli::interactive::ScriptedPrompter;

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

#[test]
fn validate_name_input_accepts_lowercase_alnum_underscore() {
    assert!(validate_name_input("agent_one").is_ok());
    assert!(validate_name_input("abc").is_ok());
}

#[test]
fn validate_name_input_rejects_short_and_uppercase() {
    assert!(validate_name_input("ab").is_err());
    assert!(validate_name_input("Agent").is_err());
    assert!(validate_name_input("has space").is_err());
}

#[test]
fn validate_port_input_enforces_non_privileged_range() {
    assert!(validate_port_input(0).is_err());
    assert!(validate_port_input(80).is_err());
    assert!(validate_port_input(8080).is_ok());
}

#[test]
fn prompt_name_retries_until_valid() {
    let prompter = scripted(&["ab", "Bad", "good_name"]);
    let name = prompt_name(&prompter).expect("eventually valid");
    assert_eq!(name, "good_name");
}

#[test]
fn prompt_name_exhausted_after_invalid_errors() {
    let prompter = scripted(&["ab"]);
    let err = prompt_name(&prompter).expect_err("no valid answer left");
    assert!(format!("{err:#}").contains("exhausted"));
}

#[test]
fn prompt_port_retries_on_unparseable_then_invalid() {
    let prompter = scripted(&["not-a-port", "80", "9000"]);
    let port = prompt_port(&prompter).expect("eventually valid");
    assert_eq!(port, 9000);
}

#[test]
fn prompt_port_uses_default_on_empty_answer() {
    let prompter = scripted(&[""]);
    let port = prompt_port(&prompter).expect("default is valid");
    assert_eq!(port, 8001);
}

#[test]
fn select_agent_from_log_files_strips_prefix_and_suffix() {
    let prompter = scripted(&["1"]);
    let files = vec!["agent-alpha.log".to_owned(), "agent-beta.log".to_owned()];
    let agent = select_agent_from_log_files(&prompter, &files, Path::new("/unused"))
        .expect("selection made");
    assert_eq!(agent, "beta");
}

#[test]
fn select_agent_from_log_files_errors_when_empty() {
    let prompter = scripted(&["0"]);
    let err =
        select_agent_from_log_files(&prompter, &[], Path::new("/unused")).expect_err("no files");
    assert!(err.to_string().contains("No agent log files found"));
}
