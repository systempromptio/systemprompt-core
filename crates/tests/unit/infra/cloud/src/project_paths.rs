//! Unit tests for `ProjectContext`, `ProjectPath`, `ProfilePath`, and
//! `UnifiedContext` covering the pure-logic path-resolution surface.

use std::fs;
use std::path::Path;

use systemprompt_cloud::paths::{
    DiscoveredProject, ProfilePath, ProjectContext, ProjectPath, UnifiedContext,
};
use tempfile::TempDir;

fn make_project(root: &Path) {
    fs::create_dir_all(root.join(".systemprompt").join("profiles").join("local"))
        .expect("create .systemprompt/profiles/local");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("write Cargo.toml");
}

#[test]
fn project_path_segments_root() {
    assert_eq!(ProjectPath::Root.segments(), &[".systemprompt"]);
}

#[test]
fn project_path_segments_profiles_dir() {
    let segs = ProjectPath::ProfilesDir.segments();
    assert_eq!(segs.len(), 2);
    assert!(segs.contains(&".systemprompt"));
    assert!(segs.contains(&"profiles"));
}

#[test]
fn project_path_segments_docker_dir() {
    let segs = ProjectPath::DockerDir.segments();
    assert_eq!(segs.len(), 2);
}

#[test]
fn project_path_segments_storage_dir() {
    let segs = ProjectPath::StorageDir.segments();
    assert!(segs.iter().any(|s| s.contains("storage")));
}

#[test]
fn project_path_segments_sessions_dir() {
    let segs = ProjectPath::SessionsDir.segments();
    assert!(segs.iter().any(|s| s.contains("session")));
}

#[test]
fn project_path_segments_dockerfile() {
    let segs = ProjectPath::Dockerfile.segments();
    assert!(segs.iter().any(|s| s.contains("Dockerfile")));
}

#[test]
fn project_path_segments_local_credentials() {
    let segs = ProjectPath::LocalCredentials.segments();
    assert!(segs.iter().any(|s| s.contains("credentials")));
}

#[test]
fn project_path_segments_local_tenants() {
    let segs = ProjectPath::LocalTenants.segments();
    assert!(segs.iter().any(|s| s.contains("tenants")));
}

#[test]
fn project_path_segments_local_session() {
    let segs = ProjectPath::LocalSession.segments();
    assert!(segs.iter().any(|s| s.contains("session")));
}

#[test]
fn project_path_is_dir_true_for_dirs() {
    assert!(ProjectPath::Root.is_dir());
    assert!(ProjectPath::ProfilesDir.is_dir());
    assert!(ProjectPath::DockerDir.is_dir());
    assert!(ProjectPath::StorageDir.is_dir());
    assert!(ProjectPath::SessionsDir.is_dir());
}

#[test]
fn project_path_is_dir_false_for_files() {
    assert!(!ProjectPath::Dockerfile.is_dir());
    assert!(!ProjectPath::LocalCredentials.is_dir());
    assert!(!ProjectPath::LocalTenants.is_dir());
    assert!(!ProjectPath::LocalSession.is_dir());
}

#[test]
fn project_path_resolve_joins_root() {
    let resolved = ProjectPath::ProfilesDir.resolve(Path::new("/proj"));
    assert!(resolved.starts_with("/proj"));
    assert!(resolved.to_string_lossy().contains("profiles"));
}

#[test]
fn project_path_debug_and_clone_copy() {
    let p = ProjectPath::Root;
    let q = p;
    assert_eq!(p, q);
    assert!(format!("{p:?}").contains("Root"));
}

#[test]
fn profile_path_filename_known() {
    let _ = ProfilePath::Config.filename();
    let _ = ProfilePath::Secrets.filename();
    let _ = ProfilePath::DockerDir.filename();
    let _ = ProfilePath::Dockerfile.filename();
    let _ = ProfilePath::Entrypoint.filename();
    let _ = ProfilePath::Dockerignore.filename();
    let _ = ProfilePath::Compose.filename();
}

#[test]
fn profile_path_is_docker_file_true() {
    assert!(ProfilePath::Dockerfile.is_docker_file());
    assert!(ProfilePath::Entrypoint.is_docker_file());
    assert!(ProfilePath::Dockerignore.is_docker_file());
    assert!(ProfilePath::Compose.is_docker_file());
}

#[test]
fn profile_path_is_docker_file_false() {
    assert!(!ProfilePath::Config.is_docker_file());
    assert!(!ProfilePath::Secrets.is_docker_file());
    assert!(!ProfilePath::DockerDir.is_docker_file());
}

#[test]
fn profile_path_resolve_config_no_docker_subdir() {
    let resolved = ProfilePath::Config.resolve(Path::new("/profile"));
    let s = resolved.to_string_lossy();
    assert!(s.starts_with("/profile/"));
    assert!(!s.contains("/docker/"));
}

#[test]
fn profile_path_resolve_dockerfile_under_docker() {
    let resolved = ProfilePath::Dockerfile.resolve(Path::new("/profile"));
    let s = resolved.to_string_lossy();
    assert!(s.contains("Dockerfile"));
}

#[test]
fn profile_path_resolve_entrypoint_under_docker() {
    let resolved = ProfilePath::Entrypoint.resolve(Path::new("/profile"));
    assert!(resolved.to_string_lossy().contains("profile"));
}

#[test]
fn profile_path_resolve_compose_under_docker() {
    let resolved = ProfilePath::Compose.resolve(Path::new("/profile"));
    assert!(resolved.to_string_lossy().contains("profile"));
}

#[test]
fn project_context_new_stores_root() {
    let p = ProjectContext::new(Path::new("/x").to_path_buf());
    assert_eq!(p.root(), Path::new("/x"));
}

#[test]
fn project_context_discover_from_finds_valid_root() {
    let tmp = TempDir::new().unwrap();
    make_project(tmp.path());
    let nested = tmp.path().join(".systemprompt").join("profiles").join("local");
    let ctx = ProjectContext::discover_from(&nested);
    assert_eq!(ctx.root(), tmp.path());
}

#[test]
fn project_context_discover_from_no_root_falls_back() {
    let tmp = TempDir::new().unwrap();
    let ctx = ProjectContext::discover_from(tmp.path());
    assert_eq!(ctx.root(), tmp.path());
}

#[test]
fn project_context_discover_returns_some_context() {
    let _ = ProjectContext::discover();
}

#[test]
fn project_context_resolve_helpers() {
    let tmp = TempDir::new().unwrap();
    let ctx = ProjectContext::new(tmp.path().to_path_buf());
    assert!(ctx.systemprompt_dir().to_string_lossy().contains(".systemprompt"));
    assert!(ctx.profiles_dir().to_string_lossy().contains("profiles"));
    let pd = ctx.profile_dir("local");
    assert!(pd.to_string_lossy().contains("local"));
    let pp = ctx.profile_path("local", ProfilePath::Config);
    assert!(pp.to_string_lossy().contains("local"));
    assert!(ctx.profile_docker_dir("local").to_string_lossy().contains("local"));
    let _ = ctx.profile_dockerfile("local");
    let _ = ctx.profile_entrypoint("local");
    let _ = ctx.profile_dockerignore("local");
    let _ = ctx.profile_compose("local");
    let _ = ctx.docker_dir();
    let _ = ctx.storage_dir();
    let _ = ctx.dockerfile();
    let _ = ctx.local_credentials();
    let _ = ctx.local_tenants();
    let _ = ctx.sessions_dir();
}

#[test]
fn project_context_exists_and_profile_exists() {
    let tmp = TempDir::new().unwrap();
    make_project(tmp.path());
    let ctx = ProjectContext::new(tmp.path().to_path_buf());
    assert!(ctx.exists(ProjectPath::Root));
    assert!(ctx.exists(ProjectPath::ProfilesDir));
    assert!(ctx.profile_exists("local"));
    assert!(!ctx.profile_exists("nonexistent_xyz"));
}

#[test]
fn project_context_debug_and_clone() {
    let ctx = ProjectContext::new(Path::new("/x").to_path_buf());
    let _ = ctx.clone();
    let d = format!("{ctx:?}");
    assert!(d.contains("ProjectContext"));
}

#[test]
fn unified_context_default_runs() {
    let _ = UnifiedContext::default();
}

#[test]
fn unified_context_discover_from_returns() {
    let tmp = TempDir::new().unwrap();
    let _ = UnifiedContext::discover_from(tmp.path());
}

#[test]
fn unified_context_no_project_paths_default() {
    let tmp = TempDir::new().unwrap();
    let ctx = UnifiedContext::discover_from(tmp.path());
    let creds = ctx.credentials_path();
    let tenants = ctx.tenants_path();
    let session = ctx.session_path();
    assert!(creds.to_string_lossy().contains("credentials"));
    assert!(tenants.to_string_lossy().contains("tenants"));
    assert!(session.to_string_lossy().contains("session"));
}

#[test]
fn unified_context_has_project_true_when_root_present() {
    let tmp = TempDir::new().unwrap();
    make_project(tmp.path());
    let ctx = UnifiedContext::discover_from(tmp.path());
    assert!(ctx.has_project());
    assert!(ctx.project().is_some());
    assert!(ctx.project_root().is_some());
    assert!(ctx.systemprompt_dir().is_some());
    assert!(ctx.profiles_dir().is_some());
    assert!(ctx.profile_dir("local").is_some());
    assert!(ctx.docker_dir().is_some());
    assert!(ctx.storage_dir().is_some());
    assert!(ctx.has_profile("local"));
    assert!(!ctx.has_profile("missing"));
}

#[test]
fn unified_context_with_profile_paths_overrides() {
    let tmp = TempDir::new().unwrap();
    let ctx = UnifiedContext::discover_from(tmp.path()).with_profile_paths(
        tmp.path(),
        "credentials.json",
        "tenants.json",
    );
    assert!(ctx.cloud_paths().is_some());
    assert!(ctx.credentials_path().to_string_lossy().contains("credentials"));
    assert!(ctx.tenants_path().to_string_lossy().contains("tenants"));
    let _ = ctx.session_path();
}

#[test]
fn unified_context_has_credentials_false_when_missing() {
    let tmp = TempDir::new().unwrap();
    let ctx = UnifiedContext::discover_from(tmp.path());
    let _ = ctx.has_credentials();
    let _ = ctx.has_tenants();
    let _ = ctx.has_session();
}

#[test]
fn unified_context_clone_debug() {
    let tmp = TempDir::new().unwrap();
    let ctx = UnifiedContext::discover_from(tmp.path());
    let _ = ctx.clone();
    let d = format!("{ctx:?}");
    assert!(d.contains("UnifiedContext"));
}

#[test]
fn discovered_project_discover_from_root() {
    let tmp = TempDir::new().unwrap();
    make_project(tmp.path());
    let proj = DiscoveredProject::discover_from(tmp.path());
    assert!(proj.is_some());
}
