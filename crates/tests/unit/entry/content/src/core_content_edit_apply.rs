//! Unit tests for `core content edit` pure field-application helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::io::Write;

use systemprompt_cli::core::content::edit::EditArgs;
use systemprompt_cli::core::content::edit_apply::{
    ContentEditState, apply_body_flags, apply_visibility_flags,
};
use systemprompt_content::CategoryIdUpdate;

fn blank_state() -> ContentEditState {
    ContentEditState {
        title: "old title".to_owned(),
        description: "old description".to_owned(),
        body: "old body".to_owned(),
        keywords: "old,keywords".to_owned(),
        image: None,
        category_id: CategoryIdUpdate::Unchanged,
        public_value: None,
        kind_value: None,
    }
}

fn args() -> EditArgs {
    EditArgs {
        identifier: None,
        source: None,
        set_values: vec![],
        public: false,
        private: false,
        body: None,
        body_file: None,
    }
}

#[test]
fn visibility_flags_noop_when_unset() {
    let mut state = blank_state();
    let mut changes = Vec::new();
    apply_visibility_flags(&args(), &mut state, &mut changes);
    assert!(changes.is_empty());
    assert!(state.public_value.is_none());
}

#[test]
fn visibility_public_flag_sets_true() {
    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.public = true;
    apply_visibility_flags(&a, &mut state, &mut changes);
    assert_eq!(state.public_value, Some(true));
    assert_eq!(changes, vec!["public: true".to_owned()]);
}

#[test]
fn visibility_private_flag_sets_false() {
    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.private = true;
    apply_visibility_flags(&a, &mut state, &mut changes);
    assert_eq!(state.public_value, Some(false));
    assert_eq!(changes, vec!["public: false".to_owned()]);
}

#[test]
fn body_flag_replaces_body() {
    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.body = Some("new body".to_owned());
    apply_body_flags(&a, &mut state, &mut changes).unwrap();
    assert_eq!(state.body, "new body");
    assert_eq!(changes, vec!["body: updated".to_owned()]);
}

#[test]
fn body_file_flag_reads_file() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "body from file").unwrap();

    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.body_file = Some(file.path().to_string_lossy().to_string());
    apply_body_flags(&a, &mut state, &mut changes).unwrap();
    assert_eq!(state.body, "body from file");
    assert_eq!(changes, vec!["body: updated from file".to_owned()]);
}

#[test]
fn body_file_missing_errors() {
    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.body_file = Some("/nonexistent/body-file.md".to_owned());
    let err = apply_body_flags(&a, &mut state, &mut changes).unwrap_err();
    assert!(err.to_string().contains("Failed to read body file"));
}

#[test]
fn body_file_takes_precedence_over_body() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "file wins").unwrap();

    let mut state = blank_state();
    let mut changes = Vec::new();
    let mut a = args();
    a.body = Some("inline".to_owned());
    a.body_file = Some(file.path().to_string_lossy().to_string());
    apply_body_flags(&a, &mut state, &mut changes).unwrap();
    assert_eq!(state.body, "file wins");
    assert_eq!(changes.len(), 2);
}
