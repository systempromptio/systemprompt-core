//! Additional tests for constants not covered in `constants.rs`:
//! container paths, `api` DB hosts, `paths` entrypoint/dockerignore/compose,
//! `credentials::VALIDATION_TTL_SECS`, and `profile` token expiry values.

use systemprompt_cloud::constants::{api, container, credentials, paths, profile};

#[test]
fn container_app_root_non_empty() {
    assert!(!container::APP.is_empty());
    assert!(!container::APP_ROOT.is_empty());
}

#[test]
fn container_app_and_app_root_equal() {
    assert_eq!(container::APP, container::APP_ROOT);
}

#[test]
fn container_bin_non_empty() {
    assert!(!container::BIN.is_empty());
}

#[test]
fn container_logs_non_empty() {
    assert!(!container::LOGS.is_empty());
}

#[test]
fn container_services_non_empty() {
    assert!(!container::SERVICES.is_empty());
}

#[test]
fn container_storage_non_empty() {
    assert!(!container::STORAGE.is_empty());
}

#[test]
fn container_web_non_empty() {
    assert!(!container::WEB.is_empty());
}

#[test]
fn container_web_dist_non_empty() {
    assert!(!container::WEB_DIST.is_empty());
}

#[test]
fn container_web_config_non_empty() {
    assert!(!container::WEB_CONFIG.is_empty());
}

#[test]
fn container_profiles_non_empty() {
    assert!(!container::PROFILES.is_empty());
}

#[test]
fn container_templates_non_empty() {
    assert!(!container::TEMPLATES.is_empty());
}

#[test]
fn container_assets_non_empty() {
    assert!(!container::ASSETS.is_empty());
}

#[test]
fn api_db_production_host_contains_systemprompt() {
    assert!(api::DB_PRODUCTION_HOST.contains("systemprompt.io"));
}

#[test]
fn api_db_sandbox_host_contains_sandbox() {
    assert!(api::DB_SANDBOX_HOST.contains("sandbox"));
}

#[test]
fn api_db_hosts_are_distinct() {
    assert_ne!(api::DB_PRODUCTION_HOST, api::DB_SANDBOX_HOST);
}

#[test]
fn credentials_validation_ttl_is_positive() {
    assert!(credentials::VALIDATION_TTL_SECS > 0);
}

#[test]
fn credentials_validation_ttl_is_15_minutes() {
    assert_eq!(credentials::VALIDATION_TTL_SECS, 900);
    assert_eq!(credentials::VALIDATION_TTL_SECS, 15 * 60);
}

#[test]
fn paths_entrypoint_non_empty() {
    assert!(!paths::ENTRYPOINT.is_empty());
}

#[test]
fn paths_dockerignore_non_empty() {
    assert!(!paths::DOCKERIGNORE.is_empty());
}

#[test]
fn paths_compose_file_non_empty() {
    assert!(!paths::COMPOSE_FILE.is_empty());
}

#[test]
fn paths_profile_docker_dir_non_empty() {
    assert!(!paths::PROFILE_DOCKER_DIR.is_empty());
}

#[test]
fn paths_profile_secrets_non_empty() {
    assert!(!paths::PROFILE_SECRETS.is_empty());
}

#[test]
fn profile_cloud_app_path_non_empty() {
    assert!(!profile::CLOUD_APP_PATH.is_empty());
}

#[test]
fn profile_credentials_path_non_empty() {
    assert!(!profile::CREDENTIALS_PATH.is_empty());
}

#[test]
fn profile_tenants_path_non_empty() {
    assert!(!profile::TENANTS_PATH.is_empty());
}

#[test]
fn profile_access_token_less_than_refresh_token() {
    assert!(profile::ACCESS_TOKEN_EXPIRATION < profile::REFRESH_TOKEN_EXPIRATION);
}

#[test]
fn profile_cloud_host_is_all_zeros() {
    assert_eq!(profile::CLOUD_HOST, "0.0.0.0");
}

#[test]
fn profile_local_host_is_loopback() {
    assert_eq!(profile::LOCAL_HOST, "127.0.0.1");
}
