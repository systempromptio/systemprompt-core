//! Unit tests for constants module

use systemprompt_cloud::constants::{
    api, checkout, cli_session, credentials, docker, env_vars, oauth, paths, profile, regions,
    tenants,
};

#[test]
fn test_api_production_url() {
    assert_eq!(api::PRODUCTION_URL, "https://api.systemprompt.io");
}

#[test]
fn test_api_sandbox_url() {
    assert_eq!(api::SANDBOX_URL, "https://api-sandbox.systemprompt.io");
}

#[test]
fn test_api_urls_use_https() {
    assert!(api::PRODUCTION_URL.starts_with("https://"));
    assert!(api::SANDBOX_URL.starts_with("https://"));
}

#[test]
fn test_oauth_callback_port() {
    assert_eq!(oauth::CALLBACK_PORT, 8765);
}

#[test]
fn test_oauth_callback_timeout() {
    assert_eq!(oauth::CALLBACK_TIMEOUT_SECS, 300);
    assert_eq!(oauth::CALLBACK_TIMEOUT_SECS, 5 * 60);
}

#[test]
fn test_checkout_callback_port() {
    assert_eq!(checkout::CALLBACK_PORT, 8766);
}

#[test]
fn test_checkout_callback_timeout() {
    assert_eq!(checkout::CALLBACK_TIMEOUT_SECS, 300);
}

#[test]
fn test_checkout_provisioning_poll_interval() {
    assert_eq!(checkout::PROVISIONING_POLL_INTERVAL_MS, 2000);
}

#[test]
fn test_oauth_and_checkout_ports_are_different() {
    assert_ne!(oauth::CALLBACK_PORT, checkout::CALLBACK_PORT);
}

#[test]
fn test_credentials_default_dir_name() {
    assert_eq!(credentials::DEFAULT_DIR_NAME, ".systemprompt");
}

#[test]
fn test_credentials_default_file_name() {
    assert_eq!(credentials::DEFAULT_FILE_NAME, "credentials.json");
}

#[test]
fn test_tenants_default_dir_name() {
    assert_eq!(tenants::DEFAULT_DIR_NAME, ".systemprompt");
}

#[test]
fn test_tenants_default_file_name() {
    assert_eq!(tenants::DEFAULT_FILE_NAME, "tenants.json");
}

#[test]
fn test_cli_session_default_dir_name() {
    assert_eq!(cli_session::DEFAULT_DIR_NAME, ".systemprompt");
}

#[test]
fn test_cli_session_default_file_name() {
    assert_eq!(cli_session::DEFAULT_FILE_NAME, "session.json");
}

#[test]
fn test_docker_container_name_prefix() {
    assert_eq!(docker::CONTAINER_NAME_PREFIX, "systemprompt-postgres");
}

#[test]
fn test_docker_compose_path() {
    assert_eq!(docker::COMPOSE_PATH, "infrastructure/docker");
}

#[test]
fn test_docker_container_name_with_env() {
    let name = docker::container_name("local");
    assert_eq!(name, "systemprompt-postgres-local");
}

#[test]
fn test_docker_container_name_with_production() {
    let name = docker::container_name("production");
    assert_eq!(name, "systemprompt-postgres-production");
}

#[test]
fn test_docker_container_name_with_empty() {
    let name = docker::container_name("");
    assert_eq!(name, "systemprompt-postgres-");
}

#[test]
fn test_docker_container_name_format() {
    let name = docker::container_name("test");
    assert!(name.starts_with(docker::CONTAINER_NAME_PREFIX));
    assert!(name.ends_with("-test"));
}

#[test]
fn test_regions_available_not_empty() {
    assert!(!regions::AVAILABLE.is_empty());
}

#[test]
fn test_regions_available_has_iad() {
    let has_iad = regions::AVAILABLE.iter().any(|(code, _)| *code == "iad");
    assert!(has_iad);
}

#[test]
fn test_regions_available_has_lhr() {
    let has_lhr = regions::AVAILABLE.iter().any(|(code, _)| *code == "lhr");
    assert!(has_lhr);
}

#[test]
fn test_regions_available_all_have_descriptions() {
    for (code, desc) in regions::AVAILABLE {
        assert!(!code.is_empty(), "Region code should not be empty");
        assert!(!desc.is_empty(), "Region description should not be empty");
    }
}

#[test]
fn test_regions_available_codes_are_lowercase() {
    for (code, _) in regions::AVAILABLE {
        assert_eq!(*code, code.to_lowercase(), "Region code should be lowercase");
    }
}

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
fn test_paths_credentials_file() {
    assert_eq!(paths::CREDENTIALS_FILE, "credentials.json");
}

#[test]
fn test_paths_tenants_file() {
    assert_eq!(paths::TENANTS_FILE, "tenants.json");
}

#[test]
fn test_paths_session_file() {
    assert_eq!(paths::SESSION_FILE, "session.json");
}

#[test]
fn test_profile_default_db_type() {
    assert_eq!(profile::DEFAULT_DB_TYPE, "postgres");
}

#[test]
fn test_profile_default_port() {
    assert_eq!(profile::DEFAULT_PORT, 8080);
}

#[test]
fn test_profile_local_host() {
    assert_eq!(profile::LOCAL_HOST, "127.0.0.1");
}

#[test]
fn test_profile_cloud_host() {
    assert_eq!(profile::CLOUD_HOST, "0.0.0.0");
}

#[test]
fn test_profile_default_cloud_url() {
    assert_eq!(profile::DEFAULT_CLOUD_URL, "https://cloud.systemprompt.io");
    assert!(profile::DEFAULT_CLOUD_URL.starts_with("https://"));
}

#[test]
fn test_profile_local_issuer() {
    assert_eq!(profile::LOCAL_ISSUER, "systemprompt-local");
}

#[test]
fn test_profile_cloud_issuer() {
    assert_eq!(profile::CLOUD_ISSUER, "systemprompt");
}

#[test]
fn test_profile_access_token_expiration() {
    assert_eq!(profile::ACCESS_TOKEN_EXPIRATION, 2_592_000);
    assert_eq!(profile::ACCESS_TOKEN_EXPIRATION, 30 * 24 * 60 * 60);
}

#[test]
fn test_profile_refresh_token_expiration() {
    assert_eq!(profile::REFRESH_TOKEN_EXPIRATION, 15_552_000);
    assert_eq!(profile::REFRESH_TOKEN_EXPIRATION, 180 * 24 * 60 * 60);
}

#[test]
fn test_env_vars_system_managed_not_empty() {
    assert!(!env_vars::SYSTEM_MANAGED.is_empty());
}

#[test]
fn test_env_vars_system_managed_contains_database_url() {
    assert!(env_vars::SYSTEM_MANAGED.contains(&"DATABASE_URL"));
}

#[test]
fn test_env_vars_system_managed_contains_jwt_secret() {
    assert!(env_vars::SYSTEM_MANAGED.contains(&"JWT_SECRET"));
}

#[test]
fn test_env_vars_cli_synced_not_empty() {
    assert!(!env_vars::CLI_SYNCED.is_empty());
}

#[test]
fn test_env_vars_cli_synced_contains_api_token() {
    assert!(env_vars::CLI_SYNCED.contains(&"SYSTEMPROMPT_API_TOKEN"));
}

#[test]
fn test_env_vars_is_system_managed_true_for_fly_vars() {
    assert!(env_vars::is_system_managed("FLY_APP_NAME"));
    assert!(env_vars::is_system_managed("FLY_MACHINE_ID"));
}

#[test]
fn test_env_vars_is_system_managed_case_insensitive() {
    assert!(env_vars::is_system_managed("fly_app_name"));
    assert!(env_vars::is_system_managed("Fly_App_Name"));
    assert!(env_vars::is_system_managed("FLY_app_name"));
}

#[test]
fn test_env_vars_is_system_managed_false_for_user_secrets() {
    assert!(!env_vars::is_system_managed("DATABASE_URL"));
    assert!(!env_vars::is_system_managed("JWT_SECRET"));
}

#[test]
fn test_env_vars_is_system_managed_false_for_custom() {
    assert!(!env_vars::is_system_managed("MY_CUSTOM_VAR"));
}

#[test]
fn test_env_vars_is_system_managed_false_for_empty() {
    assert!(!env_vars::is_system_managed(""));
}

#[test]
fn test_env_vars_is_system_managed_false_for_partial_match() {
    assert!(!env_vars::is_system_managed("DATABASE"));
    assert!(!env_vars::is_system_managed("URL"));
}
