//! Tests for `commands::cloud::profile::templates` — the small filesystem
//! helpers that have no profile/db dependency.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::templates::{
    existing_geoip_database, get_services_path, save_dockerignore, save_entrypoint,
};
use tempfile::TempDir;

#[test]
fn save_dockerignore_writes_file() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join(".dockerignore");
    save_dockerignore(&path).expect("writes");
    let content = std::fs::read_to_string(&path).expect("readable");
    assert!(content.contains(".git"));
    assert!(content.contains("target/debug"));
}

#[test]
fn save_dockerignore_creates_parent_dir() {
    let dir = TempDir::new().expect("tempdir");
    let nested = dir.path().join("a/b/c/.dockerignore");
    save_dockerignore(&nested).expect("writes");
    assert!(nested.exists());
}

#[test]
fn save_entrypoint_writes_executable_script() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("entrypoint.sh");
    save_entrypoint(&path).expect("writes");
    let content = std::fs::read_to_string(&path).expect("readable");
    assert!(content.starts_with("#!/bin/sh"));
    assert!(content.contains("systemprompt"));
    assert!(
        !content.contains("db migrate"),
        "serve migrates in-process; a migrate step here would install the schema twice: {content}"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "entrypoint should be 0755");
    }
}

#[test]
fn save_entrypoint_creates_parent_dir() {
    let dir = TempDir::new().expect("tempdir");
    let nested = dir.path().join("docker/entrypoint.sh");
    save_entrypoint(&nested).expect("writes");
    assert!(nested.exists());
}

#[test]
fn get_services_path_returns_a_path() {
    let p = get_services_path().expect("returns path");
    assert!(!p.is_empty());
}

// `existing_geoip_database` exists so re-authoring a profile carries an
// operator's hand-set path forward instead of silently wiping it, and it must
// degrade to None rather than failing when the profile is absent or corrupt.
#[test]
fn existing_geoip_database_is_none_for_a_missing_profile() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        existing_geoip_database(&tmp.path().join("absent.yaml")),
        None
    );
}

#[test]
fn existing_geoip_database_is_none_for_an_unparseable_profile() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("profile.yaml");
    std::fs::write(&path, "this: [is not: valid: profile yaml\n").unwrap();

    assert_eq!(
        existing_geoip_database(&path),
        None,
        "a corrupt profile must not abort re-authoring"
    );
}

// A hand-written profile is not a useful fixture here — Profile requires a
// dozen sections, and a parse failure would make the assertion pass for the
// wrong reason. These start from the bootstrap fixture's real profile.
fn bootstrap_profile_yaml() -> String {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    std::fs::read_to_string(&boot.profile_path).expect("bootstrap profile is readable")
}

#[test]
fn existing_geoip_database_is_none_when_the_field_is_absent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("profile.yaml");
    let yaml = bootstrap_profile_yaml();
    assert!(
        yaml.contains("geoip_database: null"),
        "the bootstrap profile is expected to declare geoip as null"
    );
    std::fs::write(&path, &yaml).unwrap();

    assert_eq!(
        existing_geoip_database(&path),
        None,
        "an explicit null must read as absent, not as an error"
    );
}

#[test]
fn existing_geoip_database_carries_a_configured_path_forward() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("profile.yaml");
    let yaml = bootstrap_profile_yaml().replace(
        "geoip_database: null",
        "geoip_database: /srv/geo/GeoLite2-City.mmdb",
    );
    std::fs::write(&path, &yaml).unwrap();

    assert_eq!(
        existing_geoip_database(&path).as_deref(),
        Some("/srv/geo/GeoLite2-City.mmdb"),
        "an operator's hand-set geoip path must survive re-authoring"
    );
}
