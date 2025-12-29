//! Unit tests for cloud constants
//!
//! Tests cover:
//! - OAuth constants (port, timeout)
//! - Checkout constants (port, timeout, poll interval)
//! - Credentials constants (dir name, file name)
//! - Tenants constants (dir name, file name)
//! - Docker constants (container prefix, compose path, container_name function)
//! - API constants (production URL, sandbox URL)
//! - Regions (available regions list)
//! - Paths constants (directory names, file names)

use systemprompt_cloud::constants::{api, checkout, credentials, docker, oauth, paths, regions, tenants};

// ============================================================================
// OAuth Constants Tests
// ============================================================================

#[test]
fn test_oauth_callback_port() {
    assert_eq!(oauth::CALLBACK_PORT, 8765);
}

#[test]
fn test_oauth_callback_timeout() {
    assert_eq!(oauth::CALLBACK_TIMEOUT_SECS, 300);
    // 5 minutes is a reasonable timeout
    assert!(oauth::CALLBACK_TIMEOUT_SECS >= 60);
}

// ============================================================================
// Checkout Constants Tests
// ============================================================================

#[test]
fn test_checkout_callback_port() {
    assert_eq!(checkout::CALLBACK_PORT, 8766);
    // Should be different from OAuth port
    assert_ne!(checkout::CALLBACK_PORT, oauth::CALLBACK_PORT);
}

#[test]
fn test_checkout_callback_timeout() {
    assert_eq!(checkout::CALLBACK_TIMEOUT_SECS, 300);
}

#[test]
fn test_checkout_polling_interval() {
    assert_eq!(checkout::PROVISIONING_POLL_INTERVAL_MS, 2000);
    // 2 seconds is reasonable for polling
    assert!(checkout::PROVISIONING_POLL_INTERVAL_MS >= 1000);
}

// ============================================================================
// Credentials Constants Tests
// ============================================================================

#[test]
fn test_credentials_default_dir_name() {
    assert_eq!(credentials::DEFAULT_DIR_NAME, ".systemprompt");
}

#[test]
fn test_credentials_default_file_name() {
    assert_eq!(credentials::DEFAULT_FILE_NAME, "credentials.json");
}

// ============================================================================
// Tenants Constants Tests
// ============================================================================

#[test]
fn test_tenants_default_dir_name() {
    assert_eq!(tenants::DEFAULT_DIR_NAME, ".systemprompt");
}

#[test]
fn test_tenants_default_file_name() {
    assert_eq!(tenants::DEFAULT_FILE_NAME, "tenants.json");
}

// ============================================================================
// Docker Constants Tests
// ============================================================================

#[test]
fn test_docker_container_name_prefix() {
    assert_eq!(docker::CONTAINER_NAME_PREFIX, "systemprompt-postgres");
}

#[test]
fn test_docker_compose_path() {
    assert_eq!(docker::COMPOSE_PATH, "infrastructure/docker");
}

#[test]
fn test_docker_container_name_function() {
    let name = docker::container_name("development");
    assert_eq!(name, "systemprompt-postgres-development");
}

#[test]
fn test_docker_container_name_with_production() {
    let name = docker::container_name("production");
    assert_eq!(name, "systemprompt-postgres-production");
}

#[test]
fn test_docker_container_name_with_staging() {
    let name = docker::container_name("staging");
    assert_eq!(name, "systemprompt-postgres-staging");
}

#[test]
fn test_docker_container_name_with_empty_env() {
    let name = docker::container_name("");
    assert_eq!(name, "systemprompt-postgres-");
}

#[test]
fn test_docker_container_name_with_special_chars() {
    let name = docker::container_name("test-env_1");
    assert_eq!(name, "systemprompt-postgres-test-env_1");
}

// ============================================================================
// API Constants Tests
// ============================================================================

#[test]
fn test_api_production_url() {
    assert_eq!(api::PRODUCTION_URL, "https://api.systemprompt.io");
}

#[test]
fn test_api_sandbox_url() {
    assert_eq!(api::SANDBOX_URL, "https://api-sandbox.systemprompt.io");
}

#[test]
fn test_api_urls_are_https() {
    assert!(api::PRODUCTION_URL.starts_with("https://"));
    assert!(api::SANDBOX_URL.starts_with("https://"));
}

#[test]
fn test_api_urls_different() {
    assert_ne!(api::PRODUCTION_URL, api::SANDBOX_URL);
}

// ============================================================================
// Regions Constants Tests
// ============================================================================

#[test]
fn test_regions_available_not_empty() {
    assert!(!regions::AVAILABLE.is_empty());
}

#[test]
fn test_regions_contains_us_east() {
    let has_us_east = regions::AVAILABLE.iter().any(|(code, _)| *code == "iad");
    assert!(has_us_east);
}

#[test]
fn test_regions_have_codes_and_names() {
    for (code, name) in regions::AVAILABLE {
        assert!(!code.is_empty());
        assert!(!name.is_empty());
    }
}

#[test]
fn test_regions_codes_are_lowercase() {
    for (code, _) in regions::AVAILABLE {
        assert_eq!(*code, code.to_lowercase());
    }
}

#[test]
fn test_regions_expected_count() {
    // Should have multiple regions available
    assert!(regions::AVAILABLE.len() >= 5);
}

#[test]
fn test_regions_contain_specific_regions() {
    let region_codes: Vec<&str> = regions::AVAILABLE.iter().map(|(c, _)| *c).collect();

    assert!(region_codes.contains(&"iad")); // US East
    assert!(region_codes.contains(&"lhr")); // London
    assert!(region_codes.contains(&"fra")); // Frankfurt
    assert!(region_codes.contains(&"sin")); // Singapore
}

// ============================================================================
// Paths Constants Tests
// ============================================================================

#[test]
fn test_paths_root_dir() {
    assert_eq!(paths::ROOT_DIR, ".systemprompt");
}

#[test]
fn test_paths_profiles_dir() {
    assert_eq!(paths::PROFILES_DIR, "profiles");
}

#[test]
fn test_paths_docker_dir() {
    assert_eq!(paths::DOCKER_DIR, "docker");
}

#[test]
fn test_paths_storage_dir() {
    assert_eq!(paths::STORAGE_DIR, "storage");
}

#[test]
fn test_paths_dockerfile() {
    assert_eq!(paths::DOCKERFILE, "Dockerfile");
}

#[test]
fn test_paths_profile_config() {
    assert_eq!(paths::PROFILE_CONFIG, "profile.yaml");
}

#[test]
fn test_paths_profile_secrets() {
    assert_eq!(paths::PROFILE_SECRETS, "secrets.json");
}

#[test]
fn test_paths_credentials_file() {
    assert_eq!(paths::CREDENTIALS_FILE, "credentials.json");
}

#[test]
fn test_paths_tenants_file() {
    assert_eq!(paths::TENANTS_FILE, "tenants.json");
}

#[test]
fn test_paths_root_is_hidden() {
    assert!(paths::ROOT_DIR.starts_with('.'));
}

#[test]
fn test_paths_config_files_are_json_or_yaml() {
    assert!(
        paths::PROFILE_CONFIG.ends_with(".yaml") || paths::PROFILE_CONFIG.ends_with(".yml")
    );
    assert!(paths::PROFILE_SECRETS.ends_with(".json"));
    assert!(paths::CREDENTIALS_FILE.ends_with(".json"));
    assert!(paths::TENANTS_FILE.ends_with(".json"));
}
