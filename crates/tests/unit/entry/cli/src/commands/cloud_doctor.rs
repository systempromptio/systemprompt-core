//! Tests for `commands::cloud::doctor::checks` — the dependency-free
//! pre-deploy checks over the local secrets map.

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::{CheckStatus, check_required_secrets};

fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn required_secrets_present_passes() {
    let secrets = map(&[("oauth_at_rest_pepper", "x"), ("database_url", "postgres://")]);
    assert_eq!(check_required_secrets(&secrets).status, CheckStatus::Pass);
}

#[test]
fn internal_database_url_satisfies_db_requirement() {
    let secrets = map(&[
        ("oauth_at_rest_pepper", "x"),
        ("internal_database_url", "postgres://"),
    ]);
    assert_eq!(check_required_secrets(&secrets).status, CheckStatus::Pass);
}

#[test]
fn missing_pepper_fails() {
    let secrets = map(&[("database_url", "postgres://")]);
    let result = check_required_secrets(&secrets);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("oauth_at_rest_pepper"));
}

#[test]
fn missing_database_url_fails() {
    let secrets = map(&[("oauth_at_rest_pepper", "x")]);
    let result = check_required_secrets(&secrets);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("database_url"));
}

#[test]
fn empty_secrets_reports_both_missing() {
    let result = check_required_secrets(&HashMap::new());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("oauth_at_rest_pepper"));
    assert!(result.detail.contains("database_url"));
}
