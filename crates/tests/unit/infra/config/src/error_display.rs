#![allow(clippy::all)]

use std::path::PathBuf;

use systemprompt_config::bootstrap::{ProfileBootstrapError, SecretsBootstrapError};
use systemprompt_config::error::ConfigError;

#[test]
fn config_error_already_initialized() {
    let e = ConfigError::AlreadyInitialized;
    assert_eq!(format!("{e}"), "Config already initialized");
}

#[test]
fn config_error_missing_profile_path() {
    let e = ConfigError::MissingProfilePath {
        field: "skills".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("paths.skills"), "got: {msg}");
}

#[test]
fn config_error_missing_system_admin() {
    let e = ConfigError::MissingSystemAdmin;
    let msg = format!("{e}");
    assert!(msg.contains("system_admin"), "got: {msg}");
}

#[test]
fn config_error_unsupported_database_type() {
    let e = ConfigError::UnsupportedDatabaseType {
        db_type: "mysql".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("mysql"), "got: {msg}");
    assert!(msg.contains("postgres"), "got: {msg}");
}

#[test]
fn config_error_invalid_database_url() {
    let e = ConfigError::InvalidDatabaseUrl(
        systemprompt_models::errors::GlobalConfigError::InvalidPostgresUrl,
    );
    let msg = format!("{e}");
    assert!(msg.starts_with("Invalid database URL: "), "got: {msg}");
}

#[test]
fn config_error_profile_path_report() {
    let e = ConfigError::ProfilePathReport {
        message: "system path missing".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("system path missing"), "got: {msg}");
}

#[test]
fn config_error_profile_path_without_parent_display() {
    let e = ConfigError::ProfilePathWithoutParent {
        path: PathBuf::from("/"),
    };
    assert_eq!(format!("{e}"), "Profile path has no parent directory: /");
}

#[test]
fn config_error_secrets_file_not_object_display() {
    let e = ConfigError::SecretsFileNotObject {
        path: PathBuf::from("/tmp/secrets.json"),
    };
    assert_eq!(
        format!("{e}"),
        "Secrets file root is not a JSON object: /tmp/secrets.json"
    );
}

#[test]
fn profile_bootstrap_error_not_initialized() {
    let e = ProfileBootstrapError::NotInitialized;
    let msg = format!("{e}");
    assert!(
        msg.contains("not initialized") || msg.contains("Not initialized"),
        "got: {msg}"
    );
}

#[test]
fn profile_bootstrap_error_already_initialized() {
    let e = ProfileBootstrapError::AlreadyInitialized;
    let msg = format!("{e}");
    assert!(
        msg.contains("already initialized") || msg.contains("Already initialized"),
        "got: {msg}"
    );
}

#[test]
fn profile_bootstrap_error_path_not_set() {
    let e = ProfileBootstrapError::PathNotSet;
    let msg = format!("{e}");
    assert!(msg.contains("SYSTEMPROMPT_PROFILE"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_not_initialized() {
    let e = SecretsBootstrapError::NotInitialized;
    let msg = format!("{e}");
    assert!(
        msg.contains("not initialized") || msg.contains("Not initialized"),
        "got: {msg}"
    );
}

#[test]
fn secrets_bootstrap_error_already_initialized() {
    let e = SecretsBootstrapError::AlreadyInitialized;
    let msg = format!("{e}");
    assert!(
        msg.contains("already initialized") || msg.contains("Already initialized"),
        "got: {msg}"
    );
}

#[test]
fn secrets_bootstrap_error_profile_not_initialized() {
    let e = SecretsBootstrapError::ProfileNotInitialized;
    let msg = format!("{e}");
    assert!(msg.contains("Profile not initialized"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_file_not_found() {
    let e = SecretsBootstrapError::FileNotFound {
        path: "/missing/secrets.json".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("/missing/secrets.json"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_invalid_secrets_file() {
    let e = SecretsBootstrapError::InvalidSecretsFile(
        systemprompt_models::errors::SecretsError::PepperTooShort { min: 32, actual: 4 },
    );
    let msg = format!("{e}");
    assert!(msg.contains("oauth_at_rest_pepper"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_no_secrets_configured() {
    let e = SecretsBootstrapError::NoSecretsConfigured;
    let msg = format!("{e}");
    assert!(!msg.is_empty());
}

#[test]
fn secrets_bootstrap_error_oauth_pepper_required() {
    let e = SecretsBootstrapError::OauthAtRestPepperRequired;
    let msg = format!("{e}");
    assert!(
        msg.contains("oauth_at_rest_pepper") || msg.contains("OAUTH_AT_REST_PEPPER"),
        "got: {msg}"
    );
}

#[test]
fn secrets_bootstrap_error_database_url_required() {
    let e = SecretsBootstrapError::DatabaseUrlRequired;
    let msg = format!("{e}");
    assert!(
        msg.contains("database_url") || msg.contains("DATABASE_URL"),
        "got: {msg}"
    );
}

#[test]
fn secrets_bootstrap_error_manifest_seed_required() {
    let e = SecretsBootstrapError::ManifestSeedRequired;
    let msg = format!("{e}");
    assert!(msg.contains("manifest_signing_secret_seed"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_manifest_seed_invalid() {
    let e =
        SecretsBootstrapError::ManifestSeedInvalid(systemprompt_config::KeyMaterialError::Length {
            expected: 32,
            actual: 3,
        });
    let msg = format!("{e}");
    assert!(msg.contains("got 3"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_signing_key_pem_required() {
    let e = SecretsBootstrapError::SigningKeyPemRequired;
    let msg = format!("{e}");
    assert!(
        msg.contains("signing_key_pem") && msg.contains("admin identity generate"),
        "got: {msg}"
    );
}
