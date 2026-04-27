//! Unit tests for ProfileLoader - loading from path and by name

use std::path::Path;
use systemprompt_loader::ProfileLoader;
use tempfile::TempDir;

// ============================================================================
// Load From Path Tests
// ============================================================================

#[test]
fn test_load_from_path_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("services"))
        .expect("Failed to create services dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");

    let profile_content = format!(
        r#"
name: test
display_name: Test Profile

site:
  name: Test Site

database:
  type: postgres

server:
  host: localhost
  port: 8080
  api_server_url: http://localhost:8080
  api_internal_url: http://localhost:8080
  api_external_url: http://localhost:8080

paths:
  system: {}
  services: {}
  bin: {}

security:
  jwt_issuer: test
  jwt_access_token_expiration: 2592000
  jwt_refresh_token_expiration: 15552000
  jwt_audiences:
    - api

rate_limits:
  disabled: true
  oauth_public_per_second: 10
  oauth_auth_per_second: 5
  contexts_per_second: 100
  tasks_per_second: 100
  artifacts_per_second: 100
  agent_registry_per_second: 10
  agents_per_second: 50
  mcp_registry_per_second: 10
  mcp_per_second: 100
  stream_per_second: 50
  content_per_second: 100
  burst_multiplier: 2
"#,
        temp_dir.path().join("system").display(),
        temp_dir.path().join("services").display(),
        temp_dir.path().join("bin").display()
    );

    let profile_path = temp_dir.path().join("test.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let profile =
        ProfileLoader::load_from_path(&profile_path).expect("should load valid profile from path");
    assert_eq!(profile.name, "test");
    assert_eq!(profile.display_name, "Test Profile");
}

#[test]
fn test_load_from_path_nonexistent() {
    let path = Path::new("/nonexistent/profile.yaml");
    let err = ProfileLoader::load_from_path(path).unwrap_err();
    assert!(err.to_string().contains("Failed to read"));
}

#[test]
fn test_load_from_path_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profile_path = temp_dir.path().join("invalid.yaml");

    std::fs::write(&profile_path, "invalid: yaml: : syntax").expect("Failed to write file");

    ProfileLoader::load_from_path(&profile_path).unwrap_err();
}

#[test]
fn test_load_from_path_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profile_path = temp_dir.path().join("incomplete.yaml");

    let content = r#"
name: incomplete
display_name: Incomplete
"#;

    std::fs::write(&profile_path, content).expect("Failed to write file");

    ProfileLoader::load_from_path(&profile_path).unwrap_err();
}

// ============================================================================
// Load By Name Tests
// ============================================================================

#[test]
fn test_load_by_name() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles_dir = temp_dir.path().join("profiles");
    std::fs::create_dir(&profiles_dir).expect("Failed to create profiles dir");

    // Create the paths directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");
    std::fs::create_dir_all(temp_dir.path().join("services"))
        .expect("Failed to create services dir");

    let profile_content = format!(
        r#"
name: dev
display_name: Development Profile

site:
  name: Dev Site

database:
  type: postgres

server:
  host: localhost
  port: 8080
  api_server_url: http://localhost:8080
  api_internal_url: http://localhost:8080
  api_external_url: http://localhost:8080

paths:
  system: {}
  services: {}
  bin: {}

security:
  jwt_issuer: dev
  jwt_access_token_expiration: 2592000
  jwt_refresh_token_expiration: 15552000
  jwt_audiences:
    - api

rate_limits:
  disabled: true
  oauth_public_per_second: 10
  oauth_auth_per_second: 5
  contexts_per_second: 100
  tasks_per_second: 100
  artifacts_per_second: 100
  agent_registry_per_second: 10
  agents_per_second: 50
  mcp_registry_per_second: 10
  mcp_per_second: 100
  stream_per_second: 50
  content_per_second: 100
  burst_multiplier: 2
"#,
        temp_dir.path().join("system").display(),
        temp_dir.path().join("services").display(),
        temp_dir.path().join("bin").display()
    );

    let profile_path = profiles_dir.join("dev.secrets.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let profile = ProfileLoader::load(temp_dir.path(), "dev").expect("should load profile by name");
    assert_eq!(profile.name, "dev");
}

#[test]
fn test_load_by_name_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles_dir = temp_dir.path().join("profiles");
    std::fs::create_dir(&profiles_dir).expect("Failed to create profiles dir");

    ProfileLoader::load(temp_dir.path(), "nonexistent").unwrap_err();
}
