//! Tests for admin-user sync profile discovery.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::cloud::auth::admin_user::{
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
    fs::write(
        c.profile_path("configured", ProfilePath::Config),
        "name: x\n",
    )
    .unwrap();
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

#[test]
fn discovery_summary_renders_found_and_skipped_profiles() {
    use systemprompt_cli::cloud::auth::admin_user::{
        ProfileDiscoveryResult, ProfileInfo, print_discovery_summary,
    };

    let result = ProfileDiscoveryResult {
        profiles: vec![ProfileInfo {
            name: "prod".to_owned(),
            display_name: None,
            database_url: Some("postgres://h/db".to_owned()),
            tenant_id: None,
            validation_mode: None,
            credentials_path: None,
            routing: None,
            is_active: None,
            session_status: None,
        }],
        skipped: vec![
            ProfileSkipReason::MissingConfig {
                path: std::path::PathBuf::from("/p/profile.yaml"),
            },
            ProfileSkipReason::MissingSecrets {
                path: std::path::PathBuf::from("/p/secrets.json"),
            },
            ProfileSkipReason::SecretsReadError {
                path: std::path::PathBuf::from("/p/secrets.json"),
                error: "denied".to_owned(),
            },
            ProfileSkipReason::SecretsParseError {
                path: std::path::PathBuf::from("/p/secrets.json"),
                error: "bad json".to_owned(),
            },
            ProfileSkipReason::MissingDatabaseUrl {
                profile: "prod".to_owned(),
            },
            ProfileSkipReason::InvalidDirectoryName {
                path: std::path::PathBuf::from("/p/\u{fffd}"),
            },
        ],
    };

    // lint-ok: no-assert both branches are console renderers with no return value
    print_discovery_summary(&result, true);
    print_discovery_summary(&result, false);

    let empty = ProfileDiscoveryResult {
        profiles: Vec::new(),
        skipped: Vec::new(),
    };
    print_discovery_summary(&empty, true);
}
