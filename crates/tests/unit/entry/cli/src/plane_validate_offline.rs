//! `admin config validate`'s config-plane pass when no configuration is
//! installed: it reports one failed `config_planes` entry rather than
//! panicking, and a targeted validation skips the pass entirely.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::plane_validate::{
    append_full_validation, plane_declaration_results,
};

#[test]
fn without_config_the_plane_pass_reports_a_single_failure() {
    let results = plane_declaration_results();
    assert_eq!(results.len(), 1);
    let only = &results[0];
    assert_eq!(only.section, "config_planes");
    assert_eq!(only.path, "config_planes");
    assert!(!only.valid);
    assert!(only.exists);
    assert!(only.error.as_deref().is_some_and(|e| !e.is_empty()));
}

#[test]
fn a_targeted_validation_skips_the_plane_pass() {
    let mut results = Vec::new();
    let target = "web".to_owned();
    assert!(append_full_validation(Some(&target), &mut results));
    assert!(results.is_empty());
}

#[test]
fn a_full_validation_without_config_fails_and_appends_the_failure() {
    let mut results = Vec::new();
    assert!(!append_full_validation(None, &mut results));
    assert_eq!(results.len(), 1);
    assert!(!results[0].valid);
}
