//! Unit tests for ProfileLoader - validation and environment variable
//! substitution

use systemprompt_loader::ProfileLoader;
use tempfile::TempDir;

// ============================================================================
// Load and Validate Tests
// ============================================================================

#[test]
fn test_load_from_path_and_validate_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create the paths directories for validation
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");
    std::fs::create_dir_all(temp_dir.path().join("services"))
        .expect("Failed to create services dir");

    let profile_content = format!(
        r#"
name: valid-test
display_name: Valid Test

site:
  name: Valid Site

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
  jwt_issuer: valid
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

    let profile_path = temp_dir.path().join("valid.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    ProfileLoader::load_from_path_and_validate(&profile_path)
        .expect("valid profile should pass validation");
}

#[test]
fn test_load_from_path_and_validate_invalid_port() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create the paths directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");
    std::fs::create_dir_all(temp_dir.path().join("services"))
        .expect("Failed to create services dir");

    let profile_content = format!(
        r#"
name: invalid-port
display_name: Invalid Port Test

site:
  name: Test Site

database:
  type: postgres

server:
  host: localhost
  port: 0
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

    let profile_path = temp_dir.path().join("invalid.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let err = ProfileLoader::load_from_path_and_validate(&profile_path).unwrap_err();
    assert!(err.to_string().contains("port"));
}

// ============================================================================
// Environment Variable Substitution Tests
// ============================================================================

#[test]
#[allow(unsafe_code)]
fn test_load_with_env_var_substitution() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("services"))
        .expect("Failed to create services dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");

    // Set environment variable for test
    unsafe { std::env::set_var("TEST_PROFILE_HOST", "env-host.example.com") };

    let profile_content = format!(
        r#"
name: env-test
display_name: Env Test Profile

site:
  name: Env Test

database:
  type: postgres

server:
  host: ${{TEST_PROFILE_HOST}}
  port: 8080
  api_server_url: http://localhost:8080
  api_internal_url: http://localhost:8080
  api_external_url: http://localhost:8080

paths:
  system: {}
  services: {}
  bin: {}

security:
  jwt_issuer: env-test
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

    let profile_path = temp_dir.path().join("env-test.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let profile = ProfileLoader::load_from_path(&profile_path)
        .expect("should load profile with env var substitution");
    assert_eq!(profile.server.host, "env-host.example.com");

    // Clean up
    unsafe { std::env::remove_var("TEST_PROFILE_HOST") };
}
