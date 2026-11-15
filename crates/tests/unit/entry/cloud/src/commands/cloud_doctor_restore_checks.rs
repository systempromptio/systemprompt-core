#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::doctor::CheckStatus;
use systemprompt_cli::cloud::doctor::restore_checks::{
    FORWARD_ONLY_HINT, check_storage_backup, schema_ahead_result,
};
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{NodeRole, StorageBackend};

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).unwrap();
    serde_yaml::from_str(&yaml).unwrap()
}

#[test]
fn single_node_local_storage_passes() {
    let mut profile = fixture_profile();
    profile.server.role = NodeRole::All;
    profile.storage.backend = StorageBackend::Local;

    let result = check_storage_backup(&profile);
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
}

#[test]
fn split_role_local_storage_warns() {
    let mut profile = fixture_profile();
    profile.server.role = NodeRole::Api;
    profile.storage.backend = StorageBackend::Local;

    let result = check_storage_backup(&profile);
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(
        result.detail.contains("point-in-time recovery"),
        "{}",
        result.detail
    );
}

#[test]
fn split_role_object_storage_passes() {
    let mut profile = fixture_profile();
    profile.server.role = NodeRole::Worker;
    profile.storage.backend = StorageBackend::S3;

    let result = check_storage_backup(&profile);
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
}

#[test]
fn schema_not_ahead_passes() {
    let result = schema_ahead_result(&[]);
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn schema_ahead_fails_with_the_forward_only_message() {
    let result = schema_ahead_result(&[("scheduler".to_owned(), vec![9, 10])]);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.detail.contains(FORWARD_ONLY_HINT),
        "{}",
        result.detail
    );
    assert!(
        result.detail.contains("scheduler [9, 10]"),
        "{}",
        result.detail
    );
}
