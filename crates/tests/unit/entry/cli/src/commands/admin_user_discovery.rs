//! Tests for admin-user sync profile discovery.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::cloud::sync::admin_user::{
    ProfileEntryResult, ProfileSkipReason, process_profile_entry,
};
use systemprompt_cloud::{ProfilePath, ProjectContext};

fn ctx(root: &Path) -> ProjectContext {
    ProjectContext::new(root.to_path_buf())
}

fn profile_dir(ctx: &ProjectContext, name: &str) -> std::path::PathBuf {
    let dir = ctx.profile_dir(name);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn non_directory_entry_is_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("stray.txt");
    fs::write(&file, "x").unwrap();

    let result = process_profile_entry(&ctx(tmp.path()), file);
    assert!(matches!(result, ProfileEntryResult::NotDirectory));
}

#[test]
fn missing_config_and_secrets_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let c = ctx(tmp.path());

    let dir = profile_dir(&c, "empty");
    let result = process_profile_entry(&c, dir);
    assert!(matches!(
        result,
        ProfileEntryResult::Skip(ProfileSkipReason::MissingConfig { .. })
    ));

    let dir = profile_dir(&c, "configured");
    fs::write(c.profile_path("configured", ProfilePath::Config), "name: x\n").unwrap();
    let result = process_profile_entry(&c, dir);
    assert!(matches!(
        result,
        ProfileEntryResult::Skip(ProfileSkipReason::MissingSecrets { .. })
    ));
}

#[test]
fn invalid_secrets_json_and_missing_database_url_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let c = ctx(tmp.path());

    let dir = profile_dir(&c, "badjson");
    fs::write(c.profile_path("badjson", ProfilePath::Config), "name: x\n").unwrap();
    fs::write(c.profile_path("badjson", ProfilePath::Secrets), "{nope").unwrap();
    assert!(matches!(
        process_profile_entry(&c, dir),
        ProfileEntryResult::Skip(ProfileSkipReason::SecretsParseError { .. })
    ));

    let dir = profile_dir(&c, "nourl");
    fs::write(c.profile_path("nourl", ProfilePath::Config), "name: x\n").unwrap();
    fs::write(c.profile_path("nourl", ProfilePath::Secrets), "{}").unwrap();
    assert!(matches!(
        process_profile_entry(&c, dir),
        ProfileEntryResult::Skip(ProfileSkipReason::MissingDatabaseUrl { .. })
    ));
}

#[test]
fn valid_profile_yields_info_with_database_url() {
    let tmp = tempfile::tempdir().unwrap();
    let c = ctx(tmp.path());

    let dir = profile_dir(&c, "prod");
    fs::write(c.profile_path("prod", ProfilePath::Config), "name: prod\n").unwrap();
    fs::write(
        c.profile_path("prod", ProfilePath::Secrets),
        "{\"database_url\": \"postgres://h/db\"}",
    )
    .unwrap();

    match process_profile_entry(&c, dir) {
        ProfileEntryResult::Valid(info) => {
            assert_eq!(info.name, "prod");
            assert_eq!(info.database_url.as_deref(), Some("postgres://h/db"));
        },
        other => panic!("expected valid profile, got {other:?}"),
    }
}
