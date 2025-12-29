//! Unit tests for ProfileLoader
//!
//! Tests cover:
//! - Profile loading from path
//! - Profile loading by name from services directory
//! - Profile validation
//! - Profile saving
//! - Available profiles listing

use std::path::Path;
use systemprompt_loader::ProfileLoader;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_valid_profile_yaml() -> String {
    r#"
name: test-profile
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
  use_https: false
  cors_allowed_origins: []

paths:
  system: /tmp/system
  core: /tmp/core
  services: /tmp/services

security:
  jwt_issuer: test-issuer
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences:
    - api
    - mcp

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

runtime:
  environment: development
  log_level: normal
  output_format: text
  no_color: false
  non_interactive: false
"#
    .to_string()
}

fn create_minimal_profile_yaml() -> String {
    r#"
name: minimal
display_name: Minimal Profile

site:
  name: Minimal Site

database:
  type: postgres

server:
  host: 0.0.0.0
  port: 3000
  api_server_url: http://localhost:3000
  api_internal_url: http://localhost:3000
  api_external_url: http://localhost:3000

paths:
  system: /tmp/system
  core: /tmp/core
  services: /tmp/services

security:
  jwt_issuer: minimal-issuer
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
"#
    .to_string()
}

// ============================================================================
// Load From Path Tests
// ============================================================================

#[test]
fn test_load_from_path_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

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
  core: {}
  services: {}

security:
  jwt_issuer: test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = temp_dir.path().join("test.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let result = ProfileLoader::load_from_path(&profile_path);
    assert!(result.is_ok());

    let profile = result.expect("Should load profile");
    assert_eq!(profile.name, "test");
    assert_eq!(profile.display_name, "Test Profile");
}

#[test]
fn test_load_from_path_nonexistent() {
    let path = Path::new("/nonexistent/profile.yaml");
    let result = ProfileLoader::load_from_path(path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_load_from_path_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profile_path = temp_dir.path().join("invalid.yaml");

    std::fs::write(&profile_path, "invalid: yaml: : syntax").expect("Failed to write file");

    let result = ProfileLoader::load_from_path(&profile_path);
    assert!(result.is_err());
}

#[test]
fn test_load_from_path_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profile_path = temp_dir.path().join("incomplete.yaml");

    // Missing most required fields
    let content = r#"
name: incomplete
display_name: Incomplete
"#;

    std::fs::write(&profile_path, content).expect("Failed to write file");

    let result = ProfileLoader::load_from_path(&profile_path);
    assert!(result.is_err());
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
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

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
  core: {}
  services: {}

security:
  jwt_issuer: dev
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = profiles_dir.join("dev.secrets.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let result = ProfileLoader::load(temp_dir.path(), "dev");
    assert!(result.is_ok());

    let profile = result.expect("Should load profile");
    assert_eq!(profile.name, "dev");
}

#[test]
fn test_load_by_name_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles_dir = temp_dir.path().join("profiles");
    std::fs::create_dir(&profiles_dir).expect("Failed to create profiles dir");

    let result = ProfileLoader::load(temp_dir.path(), "nonexistent");
    assert!(result.is_err());
}

// ============================================================================
// List Available Tests
// ============================================================================

#[test]
fn test_list_available_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles = ProfileLoader::list_available(temp_dir.path());
    assert!(profiles.is_empty());
}

#[test]
fn test_list_available_no_profiles_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles = ProfileLoader::list_available(temp_dir.path());
    assert!(profiles.is_empty());
}

#[test]
fn test_list_available_with_profiles() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles_dir = temp_dir.path().join("profiles");
    std::fs::create_dir(&profiles_dir).expect("Failed to create profiles dir");

    // Create profile files
    std::fs::write(
        profiles_dir.join("dev.secrets.profile.yaml"),
        create_valid_profile_yaml(),
    )
    .expect("Failed to write dev profile");
    std::fs::write(
        profiles_dir.join("prod.secrets.profile.yaml"),
        create_valid_profile_yaml(),
    )
    .expect("Failed to write prod profile");
    std::fs::write(
        profiles_dir.join("staging.secrets.profile.yaml"),
        create_valid_profile_yaml(),
    )
    .expect("Failed to write staging profile");

    let profiles = ProfileLoader::list_available(temp_dir.path());
    assert_eq!(profiles.len(), 3);
    assert!(profiles.contains(&"dev".to_string()));
    assert!(profiles.contains(&"prod".to_string()));
    assert!(profiles.contains(&"staging".to_string()));
}

#[test]
fn test_list_available_ignores_other_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let profiles_dir = temp_dir.path().join("profiles");
    std::fs::create_dir(&profiles_dir).expect("Failed to create profiles dir");

    // Create valid profile file
    std::fs::write(
        profiles_dir.join("dev.secrets.profile.yaml"),
        create_valid_profile_yaml(),
    )
    .expect("Failed to write profile");

    // Create other files that should be ignored
    std::fs::write(profiles_dir.join("config.yaml"), "key: value")
        .expect("Failed to write config");
    std::fs::write(profiles_dir.join("README.md"), "# Profiles").expect("Failed to write readme");
    std::fs::write(profiles_dir.join("dev.backup.yaml"), "backup: true")
        .expect("Failed to write backup");

    let profiles = ProfileLoader::list_available(temp_dir.path());
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0], "dev");
}

// ============================================================================
// Save Tests
// ============================================================================

#[test]
fn test_save_profile() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories first
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

    let profile_content = format!(
        r#"
name: save-test
display_name: Save Test Profile

site:
  name: Save Test

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
  core: {}
  services: {}

security:
  jwt_issuer: save-test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    // Load the profile first
    let profile_path = temp_dir.path().join("original.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");
    let profile = ProfileLoader::load_from_path(&profile_path).expect("Failed to load profile");

    // Save to a different location
    let save_result = ProfileLoader::save(&profile, temp_dir.path());
    assert!(save_result.is_ok());

    // Verify the saved file exists
    let saved_path = temp_dir
        .path()
        .join("profiles")
        .join("save-test.secrets.profile.yaml");
    assert!(saved_path.exists());

    // Verify the saved content includes the warning header
    let saved_content = std::fs::read_to_string(&saved_path).expect("Failed to read saved file");
    assert!(saved_content.contains("WARNING"));
    assert!(saved_content.contains("DO NOT commit"));
}

#[test]
fn test_save_creates_profiles_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

    let profile_content = format!(
        r#"
name: auto-dir
display_name: Auto Dir Profile

site:
  name: Auto Dir

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
  core: {}
  services: {}

security:
  jwt_issuer: auto-dir
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = temp_dir.path().join("profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");
    let profile = ProfileLoader::load_from_path(&profile_path).expect("Failed to load profile");

    // profiles directory doesn't exist yet
    assert!(!temp_dir.path().join("profiles").exists());

    let save_result = ProfileLoader::save(&profile, temp_dir.path());
    assert!(save_result.is_ok());

    // Now it should exist
    assert!(temp_dir.path().join("profiles").exists());
}

// ============================================================================
// Load and Validate Tests
// ============================================================================

#[test]
fn test_load_from_path_and_validate_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create the paths directories for validation
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

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
  core: {}
  services: {}

security:
  jwt_issuer: valid
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = temp_dir.path().join("valid.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let result = ProfileLoader::load_from_path_and_validate(&profile_path);
    assert!(result.is_ok());
}

#[test]
fn test_load_from_path_and_validate_invalid_port() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create the paths directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

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
  core: {}
  services: {}

security:
  jwt_issuer: test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = temp_dir.path().join("invalid.profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let result = ProfileLoader::load_from_path_and_validate(&profile_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("port"));
}

// ============================================================================
// Environment Variable Substitution Tests
// ============================================================================

#[test]
fn test_load_with_env_var_substitution() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("system")).expect("Failed to create system dir");
    std::fs::create_dir_all(temp_dir.path().join("core")).expect("Failed to create core dir");
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");

    // Set environment variable for test
    std::env::set_var("TEST_PROFILE_HOST", "env-host.example.com");

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
  core: {}
  services: {}

security:
  jwt_issuer: env-test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
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
        temp_dir.path().join("core").display(),
        temp_dir.path().join("services").display()
    );

    let profile_path = temp_dir.path().join("env-test.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");

    let result = ProfileLoader::load_from_path(&profile_path);
    assert!(result.is_ok());

    let profile = result.expect("Should load profile");
    assert_eq!(profile.server.host, "env-host.example.com");

    // Clean up
    std::env::remove_var("TEST_PROFILE_HOST");
}
