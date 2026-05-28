//! Tests for `DiscoveredProject`: walk-up project discovery + path helpers.

use std::fs;
use systemprompt_cloud::DiscoveredProject;
use tempfile::TempDir;

fn make_project_root(tmp: &TempDir) -> std::path::PathBuf {
    let root = tmp.path().to_path_buf();
    fs::create_dir_all(root.join(".systemprompt")).unwrap();
    root
}

#[test]
fn from_root_sets_paths_relative_to_root() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let p = DiscoveredProject::from_root(root.clone());
    assert_eq!(p.root(), root);
    assert_eq!(p.systemprompt_dir(), root.join(".systemprompt"));
    assert_eq!(
        p.credentials_path(),
        root.join(".systemprompt/credentials.json")
    );
    assert_eq!(p.tenants_path(), root.join(".systemprompt/tenants.json"));
    assert_eq!(p.session_path(), root.join(".systemprompt/session.json"));
}

#[test]
fn discover_from_finds_project_in_current_dir() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let p = DiscoveredProject::discover_from(&root).expect("should discover");
    assert_eq!(p.root(), root);
}

#[test]
fn discover_from_walks_up_to_parent() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let nested = root.join("nested/deeper");
    fs::create_dir_all(&nested).unwrap();
    let p = DiscoveredProject::discover_from(&nested).expect("should walk up");
    assert_eq!(p.root(), root);
}

#[test]
fn discover_from_returns_none_when_no_marker() {
    let tmp = TempDir::new().unwrap();
    let result = DiscoveredProject::discover_from(tmp.path());
    assert!(result.is_none());
}

#[test]
fn directory_helpers_return_expected_paths() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let p = DiscoveredProject::from_root(root.clone());
    assert_eq!(p.sessions_dir(), root.join(".systemprompt/sessions"));
    assert_eq!(p.profiles_dir(), root.join(".systemprompt/profiles"));
    assert_eq!(p.docker_dir(), root.join(".systemprompt/docker"));
    assert_eq!(p.storage_dir(), root.join(".systemprompt/storage"));
}

#[test]
fn profile_helpers() {
    let tmp = TempDir::new().unwrap();
    let p = DiscoveredProject::from_root(tmp.path().to_path_buf());
    assert!(
        p.profile_dir("local")
            .to_string_lossy()
            .contains("profiles/local")
    );
    assert!(
        p.profile_config("local")
            .to_string_lossy()
            .ends_with("profile.yaml")
    );
    assert!(
        p.profile_secrets("local")
            .to_string_lossy()
            .ends_with("secrets.json")
    );
}

#[test]
fn is_initialized_reflects_dir_presence() {
    let tmp = TempDir::new().unwrap();
    let uninit = DiscoveredProject::from_root(tmp.path().to_path_buf());
    assert!(!uninit.is_initialized());
    fs::create_dir_all(tmp.path().join(".systemprompt")).unwrap();
    let init = DiscoveredProject::from_root(tmp.path().to_path_buf());
    assert!(init.is_initialized());
}

#[test]
fn has_credentials_reflects_file_presence() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let p = DiscoveredProject::from_root(root.clone());
    assert!(!p.has_credentials());
    fs::write(root.join(".systemprompt/credentials.json"), "{}").unwrap();
    assert!(p.has_credentials());
}

#[test]
fn has_tenants_reflects_file_presence() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let p = DiscoveredProject::from_root(root.clone());
    assert!(!p.has_tenants());
    fs::write(root.join(".systemprompt/tenants.json"), "{}").unwrap();
    assert!(p.has_tenants());
}

#[test]
fn has_session_reflects_file_presence() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let p = DiscoveredProject::from_root(root.clone());
    assert!(!p.has_session());
    fs::write(root.join(".systemprompt/session.json"), "{}").unwrap();
    assert!(p.has_session());
}

#[test]
fn has_profile_reflects_dir_presence() {
    let tmp = TempDir::new().unwrap();
    let root = make_project_root(&tmp);
    let p = DiscoveredProject::from_root(root.clone());
    assert!(!p.has_profile("local"));
    fs::create_dir_all(root.join(".systemprompt/profiles/local")).unwrap();
    assert!(p.has_profile("local"));
}

#[test]
fn debug_includes_struct_name() {
    let tmp = TempDir::new().unwrap();
    let p = DiscoveredProject::from_root(tmp.path().to_path_buf());
    assert!(format!("{:?}", p).contains("DiscoveredProject"));
}

#[test]
fn clone_yields_equal_paths() {
    let tmp = TempDir::new().unwrap();
    let p = DiscoveredProject::from_root(tmp.path().to_path_buf());
    let c = p.clone();
    assert_eq!(p.root(), c.root());
    assert_eq!(p.systemprompt_dir(), c.systemprompt_dir());
}
