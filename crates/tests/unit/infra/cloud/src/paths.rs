//! Unit tests for CloudPath and CloudPaths

use std::path::Path;
use systemprompt_cloud::paths::{expand_home, resolve_path, CloudPath, CloudPaths};
use tempfile::TempDir;

#[test]
fn test_cloud_path_credentials_default_filename() {
    let path = CloudPath::Credentials;
    assert_eq!(path.default_filename(), "credentials.json");
}

#[test]
fn test_cloud_path_tenants_default_filename() {
    let path = CloudPath::Tenants;
    assert_eq!(path.default_filename(), "tenants.json");
}

#[test]
fn test_cloud_path_cli_session_default_filename() {
    let path = CloudPath::CliSession;
    assert_eq!(path.default_filename(), "session.json");
}

#[test]
fn test_cloud_path_sessions_dir_default_filename() {
    let path = CloudPath::SessionsDir;
    assert_eq!(path.default_filename(), "sessions");
}

#[test]
fn test_cloud_path_credentials_default_dirname() {
    let path = CloudPath::Credentials;
    assert_eq!(path.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_tenants_default_dirname() {
    let path = CloudPath::Tenants;
    assert_eq!(path.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_cli_session_default_dirname() {
    let path = CloudPath::CliSession;
    assert_eq!(path.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_sessions_dir_default_dirname() {
    let path = CloudPath::SessionsDir;
    assert_eq!(path.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_is_dir_false_for_credentials() {
    let path = CloudPath::Credentials;
    assert!(!path.is_dir());
}

#[test]
fn test_cloud_path_is_dir_false_for_tenants() {
    let path = CloudPath::Tenants;
    assert!(!path.is_dir());
}

#[test]
fn test_cloud_path_is_dir_false_for_cli_session() {
    let path = CloudPath::CliSession;
    assert!(!path.is_dir());
}

#[test]
fn test_cloud_path_is_dir_true_for_sessions_dir() {
    let path = CloudPath::SessionsDir;
    assert!(path.is_dir());
}

#[test]
fn test_cloud_path_debug() {
    let path = CloudPath::Credentials;
    let debug_str = format!("{:?}", path);
    assert!(debug_str.contains("Credentials"));
}

#[test]
fn test_cloud_path_clone() {
    let path = CloudPath::Tenants;
    let cloned = path.clone();
    assert_eq!(path, cloned);
}

#[test]
fn test_cloud_path_copy() {
    let path = CloudPath::CliSession;
    let copied = path;
    assert_eq!(path, copied);
}

#[test]
fn test_cloud_path_equality() {
    assert_eq!(CloudPath::Credentials, CloudPath::Credentials);
    assert_ne!(CloudPath::Credentials, CloudPath::Tenants);
}

#[test]
fn test_cloud_path_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(CloudPath::Credentials);
    set.insert(CloudPath::Tenants);
    set.insert(CloudPath::CliSession);
    set.insert(CloudPath::SessionsDir);

    assert_eq!(set.len(), 4);
    assert!(set.contains(&CloudPath::Credentials));
}

#[test]
fn test_cloud_paths_new() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let base = paths.base_dir();
    assert!(base.ends_with(".systemprompt"));
}

#[test]
fn test_cloud_paths_resolve_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let resolved = paths.resolve(CloudPath::Credentials);
    assert!(resolved.ends_with("credentials.json"));
}

#[test]
fn test_cloud_paths_resolve_tenants() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let resolved = paths.resolve(CloudPath::Tenants);
    assert!(resolved.ends_with("tenants.json"));
}

#[test]
fn test_cloud_paths_resolve_cli_session() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let resolved = paths.resolve(CloudPath::CliSession);
    assert!(resolved.ends_with("session.json"));
}

#[test]
fn test_cloud_paths_resolve_sessions_dir() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let resolved = paths.resolve(CloudPath::SessionsDir);
    assert!(resolved.ends_with("sessions"));
}

#[test]
fn test_cloud_paths_base_dir() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let base = paths.base_dir();
    assert!(base.starts_with(temp_dir.path()));
}

#[test]
fn test_cloud_paths_from_config_with_custom_credentials() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::from_config(
        temp_dir.path(),
        "../custom/creds.json",
        "",
    );

    let resolved = paths.resolve(CloudPath::Credentials);
    assert!(resolved.ends_with("creds.json"));
}

#[test]
fn test_cloud_paths_from_config_with_custom_tenants() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::from_config(
        temp_dir.path(),
        "",
        "../custom/tenants.json",
    );

    let resolved = paths.resolve(CloudPath::Tenants);
    assert!(resolved.ends_with("tenants.json"));
}

#[test]
fn test_cloud_paths_from_config_empty_strings_use_defaults() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::from_config(temp_dir.path(), "", "");

    let creds = paths.resolve(CloudPath::Credentials);
    let tenants = paths.resolve(CloudPath::Tenants);

    assert!(creds.ends_with("credentials.json"));
    assert!(tenants.ends_with("tenants.json"));
}

#[test]
fn test_cloud_paths_exists_false_for_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    assert!(!paths.exists(CloudPath::Credentials));
    assert!(!paths.exists(CloudPath::Tenants));
}

#[test]
fn test_cloud_paths_exists_true_for_existing() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().join(".systemprompt");
    std::fs::create_dir_all(&base_dir).unwrap();

    let creds_path = base_dir.join("credentials.json");
    std::fs::write(&creds_path, "{}").unwrap();

    let paths = CloudPaths::new(temp_dir.path());
    assert!(paths.exists(CloudPath::Credentials));
}

#[test]
fn test_cloud_paths_debug() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());

    let debug_str = format!("{:?}", paths);
    assert!(debug_str.contains("CloudPaths"));
    assert!(debug_str.contains("base_dir"));
}

#[test]
fn test_cloud_paths_clone() {
    let temp_dir = TempDir::new().unwrap();
    let paths = CloudPaths::new(temp_dir.path());
    let cloned = paths.clone();

    assert_eq!(paths.base_dir(), cloned.base_dir());
}

#[test]
fn test_resolve_path_absolute() {
    let base = Path::new("/home/user/project");
    let result = resolve_path(base, "/absolute/path");

    assert_eq!(result.to_str().unwrap(), "/absolute/path");
}

#[test]
fn test_resolve_path_relative() {
    let base = Path::new("/home/user/project");
    let result = resolve_path(base, "relative/path");

    assert!(result.starts_with("/home/user/project"));
    assert!(result.ends_with("relative/path"));
}

#[test]
fn test_resolve_path_parent_traversal() {
    let base = Path::new("/home/user/project/profiles/local");
    let result = resolve_path(base, "../../credentials.json");

    let result_str = result.to_string_lossy();
    assert!(result_str.contains("credentials.json"));
}

#[test]
fn test_expand_home_with_tilde() {
    let result = expand_home("~/test/path");

    assert!(!result.starts_with("~"));
}

#[test]
fn test_expand_home_without_tilde() {
    let result = expand_home("/absolute/path");
    assert_eq!(result.to_str().unwrap(), "/absolute/path");
}

#[test]
fn test_expand_home_relative_path() {
    let result = expand_home("relative/path");
    assert_eq!(result.to_str().unwrap(), "relative/path");
}
