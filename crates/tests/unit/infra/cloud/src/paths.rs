//! Unit tests for path utilities
//!
//! Tests cover:
//! - ProjectPath enum variants and resolve
//! - ProfilePath enum variants and resolve
//! - ProjectContext discovery and path resolution
//! - CloudPath enum and default filenames
//! - CloudPaths creation and resolution
//! - resolve_path and expand_home utilities

use std::path::{Path, PathBuf};
use systemprompt_cloud::{CloudPath, CloudPaths, ProfilePath, ProjectContext, ProjectPath, resolve_path};
use tempfile::TempDir;

// ============================================================================
// ProjectPath Tests
// ============================================================================

#[test]
fn test_project_path_root_segments() {
    let segments = ProjectPath::Root.segments();
    assert_eq!(segments, &[".systemprompt"]);
}

#[test]
fn test_project_path_profiles_dir_segments() {
    let segments = ProjectPath::ProfilesDir.segments();
    assert_eq!(segments, &[".systemprompt", "profiles"]);
}

#[test]
fn test_project_path_docker_dir_segments() {
    let segments = ProjectPath::DockerDir.segments();
    assert_eq!(segments, &[".systemprompt", "docker"]);
}

#[test]
fn test_project_path_storage_dir_segments() {
    let segments = ProjectPath::StorageDir.segments();
    assert_eq!(segments, &[".systemprompt", "storage"]);
}

#[test]
fn test_project_path_dockerfile_segments() {
    let segments = ProjectPath::Dockerfile.segments();
    assert_eq!(segments, &[".systemprompt", "Dockerfile"]);
}

#[test]
fn test_project_path_local_credentials_segments() {
    let segments = ProjectPath::LocalCredentials.segments();
    assert_eq!(segments, &[".systemprompt", "credentials.json"]);
}

#[test]
fn test_project_path_local_tenants_segments() {
    let segments = ProjectPath::LocalTenants.segments();
    assert_eq!(segments, &[".systemprompt", "tenants.json"]);
}

#[test]
fn test_project_path_is_dir() {
    assert!(ProjectPath::Root.is_dir());
    assert!(ProjectPath::ProfilesDir.is_dir());
    assert!(ProjectPath::DockerDir.is_dir());
    assert!(ProjectPath::StorageDir.is_dir());
    assert!(!ProjectPath::Dockerfile.is_dir());
    assert!(!ProjectPath::LocalCredentials.is_dir());
    assert!(!ProjectPath::LocalTenants.is_dir());
}

#[test]
fn test_project_path_resolve() {
    let root = Path::new("/project");
    let path = ProjectPath::Root.resolve(root);
    assert_eq!(path, PathBuf::from("/project/.systemprompt"));
}

#[test]
fn test_project_path_resolve_nested() {
    let root = Path::new("/home/user/project");
    let path = ProjectPath::ProfilesDir.resolve(root);
    assert_eq!(path, PathBuf::from("/home/user/project/.systemprompt/profiles"));
}

// ============================================================================
// ProfilePath Tests
// ============================================================================

#[test]
fn test_profile_path_config_filename() {
    assert_eq!(ProfilePath::Config.filename(), "profile.yaml");
}

#[test]
fn test_profile_path_secrets_filename() {
    assert_eq!(ProfilePath::Secrets.filename(), "secrets.json");
}

#[test]
fn test_profile_path_config_resolve() {
    let profile_dir = Path::new("/profiles/dev");
    let path = ProfilePath::Config.resolve(profile_dir);
    assert_eq!(path, PathBuf::from("/profiles/dev/profile.yaml"));
}

#[test]
fn test_profile_path_secrets_resolve() {
    let profile_dir = Path::new("/profiles/prod");
    let path = ProfilePath::Secrets.resolve(profile_dir);
    assert_eq!(path, PathBuf::from("/profiles/prod/secrets.json"));
}

// ============================================================================
// ProjectContext Tests
// ============================================================================

#[test]
fn test_project_context_new() {
    let root = PathBuf::from("/project/root");
    let ctx = ProjectContext::new(root.clone());
    assert_eq!(ctx.root(), root.as_path());
}

#[test]
fn test_project_context_discover_from_with_systemprompt_dir() {
    let temp_dir = TempDir::new().unwrap();
    let systemprompt_dir = temp_dir.path().join(".systemprompt");
    std::fs::create_dir_all(&systemprompt_dir).unwrap();

    let ctx = ProjectContext::discover_from(temp_dir.path());
    assert_eq!(ctx.root(), temp_dir.path());
}

#[test]
fn test_project_context_discover_from_subdirectory() {
    let temp_dir = TempDir::new().unwrap();
    let systemprompt_dir = temp_dir.path().join(".systemprompt");
    std::fs::create_dir_all(&systemprompt_dir).unwrap();

    let subdir = temp_dir.path().join("src").join("components");
    std::fs::create_dir_all(&subdir).unwrap();

    let ctx = ProjectContext::discover_from(&subdir);
    assert_eq!(ctx.root(), temp_dir.path());
}

#[test]
fn test_project_context_discover_from_no_systemprompt_dir() {
    let temp_dir = TempDir::new().unwrap();
    let start = temp_dir.path().join("some").join("path");
    std::fs::create_dir_all(&start).unwrap();

    let ctx = ProjectContext::discover_from(&start);
    // Should fall back to start path when no .systemprompt found
    assert_eq!(ctx.root(), start.as_path());
}

#[test]
fn test_project_context_systemprompt_dir() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.systemprompt_dir(), PathBuf::from("/project/.systemprompt"));
}

#[test]
fn test_project_context_profiles_dir() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.profiles_dir(), PathBuf::from("/project/.systemprompt/profiles"));
}

#[test]
fn test_project_context_profile_dir() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.profile_dir("development"), PathBuf::from("/project/.systemprompt/profiles/development"));
}

#[test]
fn test_project_context_profile_path() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    let config_path = ctx.profile_path("production", ProfilePath::Config);
    assert_eq!(config_path, PathBuf::from("/project/.systemprompt/profiles/production/profile.yaml"));
}

#[test]
fn test_project_context_docker_dir() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.docker_dir(), PathBuf::from("/project/.systemprompt/docker"));
}

#[test]
fn test_project_context_storage_dir() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.storage_dir(), PathBuf::from("/project/.systemprompt/storage"));
}

#[test]
fn test_project_context_dockerfile() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.dockerfile(), PathBuf::from("/project/.systemprompt/Dockerfile"));
}

#[test]
fn test_project_context_local_credentials() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.local_credentials(), PathBuf::from("/project/.systemprompt/credentials.json"));
}

#[test]
fn test_project_context_local_tenants() {
    let ctx = ProjectContext::new(PathBuf::from("/project"));
    assert_eq!(ctx.local_tenants(), PathBuf::from("/project/.systemprompt/tenants.json"));
}

#[test]
fn test_project_context_exists() {
    let temp_dir = TempDir::new().unwrap();
    let systemprompt_dir = temp_dir.path().join(".systemprompt");
    std::fs::create_dir_all(&systemprompt_dir).unwrap();

    let ctx = ProjectContext::new(temp_dir.path().to_path_buf());
    assert!(ctx.exists(ProjectPath::Root));
    assert!(!ctx.exists(ProjectPath::ProfilesDir));
}

#[test]
fn test_project_context_profile_exists() {
    let temp_dir = TempDir::new().unwrap();
    let profile_dir = temp_dir.path().join(".systemprompt").join("profiles").join("dev");
    std::fs::create_dir_all(&profile_dir).unwrap();

    let ctx = ProjectContext::new(temp_dir.path().to_path_buf());
    assert!(ctx.profile_exists("dev"));
    assert!(!ctx.profile_exists("prod"));
}

// ============================================================================
// CloudPath Tests
// ============================================================================

#[test]
fn test_cloud_path_credentials_default_filename() {
    assert_eq!(CloudPath::Credentials.default_filename(), "credentials.json");
}

#[test]
fn test_cloud_path_tenants_default_filename() {
    assert_eq!(CloudPath::Tenants.default_filename(), "tenants.json");
}

#[test]
fn test_cloud_path_credentials_default_dirname() {
    assert_eq!(CloudPath::Credentials.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_tenants_default_dirname() {
    assert_eq!(CloudPath::Tenants.default_dirname(), ".systemprompt");
}

#[test]
fn test_cloud_path_equality() {
    assert_eq!(CloudPath::Credentials, CloudPath::Credentials);
    assert_eq!(CloudPath::Tenants, CloudPath::Tenants);
    assert_ne!(CloudPath::Credentials, CloudPath::Tenants);
}

// ============================================================================
// CloudPaths Tests
// ============================================================================

#[test]
fn test_cloud_paths_new() {
    let paths = CloudPaths::new(Path::new("/profiles/dev"));
    assert_eq!(paths.base_dir(), Path::new("/profiles/dev/.systemprompt"));
}

#[test]
fn test_cloud_paths_resolve_credentials() {
    let paths = CloudPaths::new(Path::new("/profiles/dev"));
    let creds_path = paths.resolve(CloudPath::Credentials);
    assert_eq!(creds_path, PathBuf::from("/profiles/dev/.systemprompt/credentials.json"));
}

#[test]
fn test_cloud_paths_resolve_tenants() {
    let paths = CloudPaths::new(Path::new("/profiles/dev"));
    let tenants_path = paths.resolve(CloudPath::Tenants);
    assert_eq!(tenants_path, PathBuf::from("/profiles/dev/.systemprompt/tenants.json"));
}

#[test]
fn test_cloud_paths_from_config() {
    let paths = CloudPaths::from_config(
        Path::new("/base"),
        "custom/credentials.json",
        "custom/tenants.json",
    );

    let creds_path = paths.resolve(CloudPath::Credentials);
    let tenants_path = paths.resolve(CloudPath::Tenants);

    assert_eq!(creds_path, PathBuf::from("/base/custom/credentials.json"));
    assert_eq!(tenants_path, PathBuf::from("/base/custom/tenants.json"));
}

#[test]
fn test_cloud_paths_exists() {
    let temp_dir = TempDir::new().unwrap();
    let systemprompt_dir = temp_dir.path().join(".systemprompt");
    std::fs::create_dir_all(&systemprompt_dir).unwrap();

    let creds_path = systemprompt_dir.join("credentials.json");
    std::fs::write(&creds_path, "{}").unwrap();

    let paths = CloudPaths::new(temp_dir.path());

    assert!(paths.exists(CloudPath::Credentials));
    assert!(!paths.exists(CloudPath::Tenants));
}

// ============================================================================
// resolve_path Tests
// ============================================================================

#[test]
fn test_resolve_path_relative() {
    let result = resolve_path(Path::new("/base"), "relative/path");
    assert_eq!(result, PathBuf::from("/base/relative/path"));
}

#[test]
fn test_resolve_path_absolute() {
    let result = resolve_path(Path::new("/base"), "/absolute/path");
    assert_eq!(result, PathBuf::from("/absolute/path"));
}

#[test]
fn test_resolve_path_with_dot() {
    let result = resolve_path(Path::new("/base"), "./here");
    assert_eq!(result, PathBuf::from("/base/./here"));
}

// ============================================================================
// Debug Trait Tests
// ============================================================================

#[test]
fn test_project_path_debug() {
    let debug_str = format!("{:?}", ProjectPath::Root);
    assert!(debug_str.contains("Root"));
}

#[test]
fn test_profile_path_debug() {
    let debug_str = format!("{:?}", ProfilePath::Config);
    assert!(debug_str.contains("Config"));
}

#[test]
fn test_cloud_path_debug() {
    let debug_str = format!("{:?}", CloudPath::Credentials);
    assert!(debug_str.contains("Credentials"));
}

#[test]
fn test_project_context_debug() {
    let ctx = ProjectContext::new(PathBuf::from("/test"));
    let debug_str = format!("{:?}", ctx);
    assert!(debug_str.contains("ProjectContext"));
}

#[test]
fn test_cloud_paths_debug() {
    let paths = CloudPaths::new(Path::new("/test"));
    let debug_str = format!("{:?}", paths);
    assert!(debug_str.contains("CloudPaths"));
}
