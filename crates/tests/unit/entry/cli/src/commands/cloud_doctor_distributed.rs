//! Tests for the pure `cloud doctor --distributed` checks: identity
//! fingerprints, instance id and trusted proxies. The database and readiness
//! probes are network-dependent and covered by the integration harness.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::CheckStatus;
use systemprompt_cli::cloud::doctor::distributed::{
    check_identity_fingerprints, check_instance_id, check_trusted_proxies,
};
use systemprompt_models::Profile;

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).unwrap();
    serde_yaml::from_str(&yaml).unwrap()
}

fn secrets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn identity_fingerprints_pass_and_are_stable_for_equal_values() {
    let all = secrets(&[
        ("oauth_at_rest_pepper", "pepper-value"),
        ("manifest_signing_secret_seed", "seed-value"),
        ("signing_key_pem", "pem-value"),
    ]);
    let first = check_identity_fingerprints(&all);
    let second = check_identity_fingerprints(&all);
    assert_eq!(first.status, CheckStatus::Pass, "{}", first.detail);
    assert_eq!(
        first.detail, second.detail,
        "same secrets must print the same fingerprints"
    );
    assert!(
        !first.detail.contains("pepper-value"),
        "raw secrets never appear in the report"
    );

    let mut other = all.clone();
    other.insert("signing_key_pem".to_owned(), "different".to_owned());
    assert_ne!(check_identity_fingerprints(&other).detail, first.detail);
}

#[test]
fn identity_fingerprints_fail_when_a_secret_is_missing() {
    let partial = secrets(&[("oauth_at_rest_pepper", "pepper-value")]);
    let result = check_identity_fingerprints(&partial);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.detail.contains("manifest_signing_secret_seed"));
    assert!(result.detail.contains("signing_key_pem"));
    assert!(result.detail.contains("admin identity generate"));
}

#[test]
fn instance_id_warns_when_unset_and_passes_when_explicit() {
    let mut profile = fixture_profile();
    profile.server.instance_id = None;
    assert_eq!(check_instance_id(&profile).status, CheckStatus::Warn);
    profile.server.instance_id = Some("node-a".to_owned());
    let result = check_instance_id(&profile);
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.detail.contains("node-a"));
}

#[test]
fn trusted_proxies_fail_when_empty() {
    let mut profile = fixture_profile();
    profile.server.trusted_proxies = Vec::new();
    assert_eq!(check_trusted_proxies(&profile).status, CheckStatus::Fail);
    profile.server.trusted_proxies = vec!["fc00::/7".parse().unwrap()];
    assert_eq!(check_trusted_proxies(&profile).status, CheckStatus::Pass);
}
