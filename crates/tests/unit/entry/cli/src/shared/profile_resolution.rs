//! Tests for the actual cli `profile` resolution helpers — `is_path_input`,
//! `resolve_profile_from_path`, and `resolve_profile_path`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::{is_path_input, resolve_profile_from_path, resolve_profile_path};
use tempfile::TempDir;

#[test]
fn is_path_input_yaml_extension() {
    assert!(is_path_input("foo.yaml"));
    assert!(is_path_input("foo.yml"));
    assert!(is_path_input("FOO.YAML"));
}

#[test]
fn is_path_input_contains_slash() {
    assert!(is_path_input("a/b"));
    assert!(is_path_input("./foo"));
    assert!(is_path_input("../foo"));
}

#[test]
fn is_path_input_starts_with_tilde() {
    assert!(is_path_input("~/profile"));
    assert!(is_path_input("~"));
}

#[test]
fn is_path_input_plain_name_is_false() {
    assert!(!is_path_input("local"));
    assert!(!is_path_input("production"));
    assert!(!is_path_input("my_profile"));
}

#[test]
fn resolve_profile_from_path_existing_yaml() {
    let dir = TempDir::new().expect("tempdir");
    let yaml = dir.path().join("profile.yaml");
    std::fs::write(&yaml, "name: test\n").expect("write");
    let resolved = resolve_profile_from_path(yaml.to_str().expect("utf8")).expect("resolves");
    assert_eq!(resolved, yaml);
}

#[test]
fn resolve_profile_from_path_returns_dir_when_dir_exists() {
    let dir = TempDir::new().expect("tempdir");
    let yaml = dir.path().join("profile.yaml");
    std::fs::write(&yaml, "name: test\n").expect("write");
    let resolved = resolve_profile_from_path(dir.path().to_str().expect("utf8")).expect("resolves");
    assert_eq!(resolved, dir.path());
}

#[test]
fn resolve_profile_from_path_nonexistent_dir_falls_back_to_inner_yaml() {
    let dir = TempDir::new().expect("tempdir");
    let sub = dir.path().join("does-not-exist-as-dir");
    std::fs::create_dir(&sub).expect("mkdir");
    let yaml = sub.join("profile.yaml");
    std::fs::write(&yaml, "name: test\n").expect("write");
    let resolved = resolve_profile_from_path(sub.to_str().expect("utf8")).expect("resolves");
    assert_eq!(resolved, sub);
}

#[test]
fn resolve_profile_from_path_missing_errors() {
    let err = resolve_profile_from_path("/definitely/not/a/real/path/please/12345").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not found"));
}

#[test]
fn resolve_profile_path_passes_through_path_override() {
    let dir = TempDir::new().expect("tempdir");
    let yaml = dir.path().join("profile.yaml");
    std::fs::write(&yaml, "name: t\n").expect("write");
    let resolved =
        resolve_profile_path(Some(yaml.to_str().expect("utf8")), None).expect("resolves");
    assert_eq!(resolved, yaml);
}

#[test]
fn resolve_profile_path_uses_session_path_when_exists() {
    if std::env::var("SYSTEMPROMPT_PROFILE").is_ok() {
        // env var takes precedence over `from_session`; skip this scenario
        // when the parent shell has it set.
        return;
    }
    let dir = TempDir::new().expect("tempdir");
    let yaml = dir.path().join("profile.yaml");
    std::fs::write(&yaml, "name: t\n").expect("write");
    let resolved = resolve_profile_path(None, Some(yaml.clone())).expect("resolves");
    assert_eq!(resolved, yaml);
}
