//! Unit tests for ProfileLoader - listing available profiles and saving

use systemprompt_loader::ProfileLoader;
use tempfile::TempDir;

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
  services: /tmp/services
  bin: /tmp/bin

security:
  jwt_issuer: test-issuer
  jwt_access_token_expiration: 2592000
  jwt_refresh_token_expiration: 15552000
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
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");
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
  services: {}
  bin: {}

security:
  jwt_issuer: save-test
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

    // Load the profile first
    let profile_path = temp_dir.path().join("original.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");
    let profile = ProfileLoader::load_from_path(&profile_path).expect("Failed to load profile");

    // Save to a different location
    ProfileLoader::save(&profile, temp_dir.path())
        .expect("should save profile");

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
    std::fs::create_dir_all(temp_dir.path().join("services")).expect("Failed to create services dir");
    std::fs::create_dir_all(temp_dir.path().join("bin")).expect("Failed to create bin dir");

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
  services: {}
  bin: {}

security:
  jwt_issuer: auto-dir
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

    let profile_path = temp_dir.path().join("profile.yaml");
    std::fs::write(&profile_path, profile_content).expect("Failed to write profile");
    let profile = ProfileLoader::load_from_path(&profile_path).expect("Failed to load profile");

    // profiles directory doesn't exist yet
    assert!(!temp_dir.path().join("profiles").exists());

    ProfileLoader::save(&profile, temp_dir.path())
        .expect("should save profile and create directory");

    // Now it should exist
    assert!(temp_dir.path().join("profiles").exists());
}
