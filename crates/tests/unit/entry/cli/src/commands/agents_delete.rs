//! Tests for `admin::agents::delete` — target resolution and confirmation
//! messaging.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::agents::delete::{
    delete_confirm_message, delete_success_message, validate_delete_targets,
};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn named_target_must_exist() {
    let err = validate_delete_targets(Some("ghost".to_owned()), &names(&["a", "b"]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("'ghost' not found"));
}

#[test]
fn named_target_resolves_to_single_agent() {
    let targets = validate_delete_targets(Some("a".to_owned()), &names(&["a", "b"])).unwrap();
    assert_eq!(targets, vec!["a"]);
}

#[test]
fn all_resolves_to_every_agent() {
    let targets = validate_delete_targets(None, &names(&["a", "b"])).unwrap();
    assert_eq!(targets, vec!["a", "b"]);
}

#[test]
fn all_with_no_agents_errors() {
    let err = validate_delete_targets(None, &[]).unwrap_err().to_string();
    assert!(err.contains("No agents to delete"));
}

#[test]
fn confirm_message_for_all_counts_targets() {
    assert_eq!(
        delete_confirm_message(true, &names(&["a", "b", "c"])),
        "Delete ALL 3 agents?"
    );
}

#[test]
fn confirm_message_for_single_names_agent() {
    assert_eq!(
        delete_confirm_message(false, &names(&["worker"])),
        "Delete agent 'worker'?"
    );
}

#[test]
fn success_message_singular_and_plural() {
    assert_eq!(
        delete_success_message(&names(&["worker"])),
        "Agent 'worker' deleted successfully"
    );
    assert_eq!(
        delete_success_message(&names(&["a", "b"])),
        "2 agent(s) deleted successfully"
    );
    assert_eq!(delete_success_message(&[]), "0 agent(s) deleted successfully");
}
