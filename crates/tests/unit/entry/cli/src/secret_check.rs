//! `admin config secret check` reports names, never values.
//!
//! The command exists to diagnose a failing boot, which is exactly when it is
//! most likely to be run on a shared screen, so the absence of values in the
//! report is the property under test.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::secret_check::{
    SecretCheckReport, file_key_names, missing_required,
};

const SECRET_VALUE: &str = "sk-do-not-print-this";

fn secrets_file(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(&path, body).expect("write");
    (dir, path)
}

#[test]
fn key_names_are_sorted_and_values_are_not_returned() {
    let (_dir, path) = secrets_file(&format!(
        r#"{{"oauth_at_rest_pepper":"p","anthropic":"{SECRET_VALUE}","database_url":"postgres://x"}}"#
    ));
    let keys = file_key_names(&path).expect("file parses");
    assert_eq!(
        keys,
        vec!["anthropic", "database_url", "oauth_at_rest_pepper"]
    );
    assert!(!keys.iter().any(|k| k.contains("sk-")));
}

#[test]
fn a_rendered_report_never_carries_a_value() {
    let (_dir, path) = secrets_file(&format!(
        r#"{{"oauth_at_rest_pepper":"p","anthropic":"{SECRET_VALUE}","database_url":"postgres://x"}}"#
    ));
    let keys = file_key_names(&path).expect("file parses");
    let report = SecretCheckReport {
        source: "file".to_owned(),
        detail: path.display().to_string(),
        missing_required: missing_required(&keys),
        keys,
    };

    let rendered = serde_json::to_string(&report).expect("report serialises");
    assert!(rendered.contains("anthropic"), "key name is missing");
    assert!(
        !rendered.contains(SECRET_VALUE),
        "a secret value reached the report: {rendered}"
    );
    assert!(!rendered.contains("postgres://x"), "a database url leaked");
}

#[test]
fn a_missing_pepper_is_reported() {
    let missing = missing_required(&["database_url".to_owned()]);
    assert_eq!(missing, vec!["oauth_at_rest_pepper".to_owned()]);
}

#[test]
fn an_internal_database_url_satisfies_the_database_requirement() {
    let missing = missing_required(&[
        "oauth_at_rest_pepper".to_owned(),
        "internal_database_url".to_owned(),
    ]);
    assert!(missing.is_empty(), "unexpected gaps: {missing:?}");
}

#[test]
fn an_upper_case_key_still_counts_as_present() {
    let missing = missing_required(&["OAUTH_AT_REST_PEPPER".to_owned(), "DATABASE_URL".to_owned()]);
    assert!(missing.is_empty(), "unexpected gaps: {missing:?}");
}

#[test]
fn a_document_with_no_keys_reports_both_gaps() {
    let missing = missing_required(&[]);
    assert_eq!(missing.len(), 2);
    assert!(missing.iter().any(|m| m.contains("internal_database_url")));
}
