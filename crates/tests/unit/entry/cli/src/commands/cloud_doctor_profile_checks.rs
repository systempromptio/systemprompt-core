//! Tests for the deploy-preflight checks that take a whole `Profile`.
//!
//! `cloud_doctor` covers the secrets and proxy checks; these drive the four
//! profile-shaped checks, which no test called, against the bootstrap
//! fixture's profile and mutated copies of it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use systemprompt_cli::cloud::doctor::{
    CheckStatus, check_extension_configs, check_profile_valid, check_provider_secrets,
    check_signing_key,
};
use systemprompt_models::Profile;
use systemprompt_models::services::ProviderRegistry;

fn fixture_profile() -> (Profile, PathBuf) {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).unwrap();
    let profile: Profile = serde_yaml::from_str(&yaml).unwrap();
    let dir = boot.profile_path.parent().unwrap().to_path_buf();
    (profile, dir)
}

fn secrets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn a_bootstrapped_profile_passes_schema_validation() {
    let (profile, _dir) = fixture_profile();

    let result = check_profile_valid(&profile);
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
    assert_eq!(result.name, "profile");
}

#[test]
fn a_profile_with_an_empty_name_fails_validation() {
    let (mut profile, _dir) = fixture_profile();
    profile.name = String::new();

    let result = check_profile_valid(&profile);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(!result.detail.is_empty());
}

#[test]
fn extension_configs_are_checked_against_the_services_tree() {
    let (profile, _dir) = fixture_profile();

    let result = check_extension_configs(&profile);
    assert_eq!(result.name, "extension-config");
    assert_ne!(result.detail, "");
}

#[test]
fn a_signing_key_supplied_through_secrets_short_circuits_the_file_check() {
    let (profile, dir) = fixture_profile();

    let result = check_signing_key(
        &profile,
        &dir,
        &secrets(&[("signing_key_pem", "-----BEGIN PRIVATE KEY-----")]),
    );
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.detail.contains("secrets.json"));
}

#[test]
fn an_existing_key_file_passes_and_a_missing_one_explains_the_remedy() {
    let (profile, _dir) = fixture_profile();
    let tmp = tempfile::tempdir().unwrap();

    let missing = check_signing_key(&profile, tmp.path(), &secrets(&[]));
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.detail.contains("admin keys generate"));

    let key_path = tmp.path().join(&profile.security.signing_key_path);
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&key_path, "pem").unwrap();

    let present = check_signing_key(&profile, tmp.path(), &secrets(&[]));
    assert_eq!(present.status, CheckStatus::Pass, "{}", present.detail);
    assert!(present.detail.contains(&key_path.display().to_string()));
}

#[test]
fn an_absolute_signing_key_path_ignores_the_profile_directory() {
    let (mut profile, _dir) = fixture_profile();
    let tmp = tempfile::tempdir().unwrap();
    let absolute = tmp.path().join("absolute_key.pem");
    std::fs::write(&absolute, "pem").unwrap();
    profile.security.signing_key_path = absolute.clone();

    let result = check_signing_key(&profile, Path::new("/nonexistent"), &secrets(&[]));
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.detail.contains(&absolute.display().to_string()));
}

#[test]
fn provider_credentials_are_reported_per_missing_secret() {
    let empty = ProviderRegistry::default();
    let none_declared = check_provider_secrets(&empty, &secrets(&[]));
    assert_eq!(none_declared.name, "providers");
    assert_eq!(none_declared.status, CheckStatus::Pass);

    let registry = ProviderRegistry::default_seed().unwrap();
    let result = check_provider_secrets(&registry, &secrets(&[]));
    assert_eq!(result.status, CheckStatus::Fail);
    for provider in &registry.providers {
        assert!(
            result.detail.contains(provider.name.as_str()),
            "{}",
            result.detail
        );
    }

    let all_present = secrets(
        &registry
            .providers
            .iter()
            .map(|p| (p.api_key_secret.as_str(), "value"))
            .collect::<Vec<_>>(),
    );
    let satisfied = check_provider_secrets(&registry, &all_present);
    assert_eq!(satisfied.status, CheckStatus::Pass, "{}", satisfied.detail);
}
