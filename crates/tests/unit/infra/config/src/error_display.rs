#![allow(clippy::all)]

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
    let e = ConfigError::InvalidDatabaseUrl {
        message: "missing host".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("missing host"), "got: {msg}");
}

#[test]
fn config_error_unresolved_variables() {
    let e = ConfigError::UnresolvedVariables {
        passes: 3,
        unresolved: "${FOO}".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("3"), "got: {msg}");
    assert!(msg.contains("${FOO}"), "got: {msg}");
}

#[test]
fn config_error_validation_errors() {
    let e = ConfigError::ValidationErrors { count: 5 };
    let msg = format!("{e}");
    assert!(msg.contains("5"), "got: {msg}");
}

#[test]
fn config_error_environment_config_missing() {
    let e = ConfigError::EnvironmentConfigMissing {
        path: std::path::PathBuf::from("/some/path"),
    };
    let msg = format!("{e}");
    assert!(msg.contains("/some/path"), "got: {msg}");
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
fn config_error_other_helper() {
    let e = ConfigError::other("something went wrong");
    let msg = format!("{e}");
    assert!(msg.contains("something went wrong"), "got: {msg}");
}

#[test]
fn config_error_other_variant_display() {
    let e = ConfigError::Other {
        message: "test message".to_owned(),
    };
    assert_eq!(format!("{e}"), "test message");
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
fn profile_bootstrap_error_validation_failed() {
    let e = ProfileBootstrapError::ValidationFailed("bad config".to_owned());
    let msg = format!("{e}");
    assert!(msg.contains("bad config"), "got: {msg}");
}

#[test]
fn profile_bootstrap_error_load_failed() {
    let e = ProfileBootstrapError::LoadFailed("io error".to_owned());
    let msg = format!("{e}");
    assert!(msg.contains("io error"), "got: {msg}");
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
    let e = SecretsBootstrapError::InvalidSecretsFile {
        message: "bad JSON".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("bad JSON"), "got: {msg}");
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
fn secrets_bootstrap_error_manifest_seed_unavailable() {
    let e = SecretsBootstrapError::ManifestSeedUnavailable;
    let msg = format!("{e}");
    assert!(msg.contains("manifest_signing_secret_seed"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_manifest_seed_invalid() {
    let e = SecretsBootstrapError::ManifestSeedInvalid {
        message: "decode failed".to_owned(),
    };
    let msg = format!("{e}");
    assert!(msg.contains("decode failed"), "got: {msg}");
}

#[test]
fn secrets_bootstrap_error_subprocess_seed_missing() {
    let e = SecretsBootstrapError::SubprocessSeedMissing;
    let msg = format!("{e}");
    assert!(
        msg.contains("subprocess") || msg.contains("MANIFEST_SIGNING_SECRET_SEED"),
        "got: {msg}"
    );
}
