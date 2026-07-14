use systemprompt_config::{ConfigError, ProfileBootstrap, ProfileBootstrapError};

use crate::fixture;

#[test]
fn init_reads_path_from_systemprompt_profile_env() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    fixture::set_env(
        "SYSTEMPROMPT_PROFILE",
        &fx.profile_path.display().to_string(),
    );

    let profile = ProfileBootstrap::init().unwrap();

    assert_eq!(profile.name, "config_fixture");
    assert_eq!(
        ProfileBootstrap::get_path().unwrap(),
        fx.profile_path.display().to_string()
    );
    assert!(ProfileBootstrap::is_initialized());
}

#[test]
fn init_errors_when_env_var_unset() {
    fixture::remove_env("SYSTEMPROMPT_PROFILE");

    let err = ProfileBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Profile(ProfileBootstrapError::PathNotSet)
    ));
}

#[test]
fn init_errors_when_profile_file_missing() {
    fixture::set_env("SYSTEMPROMPT_PROFILE", "/nonexistent/profile.yaml");

    let err = ProfileBootstrap::init().unwrap_err();

    assert!(matches!(err, ConfigError::ProfileParse(_)), "got: {err:?}");
}

#[test]
fn init_after_init_from_path_is_already_initialized() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env(
        "SYSTEMPROMPT_PROFILE",
        &fx.profile_path.display().to_string(),
    );

    let err = ProfileBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Profile(ProfileBootstrapError::AlreadyInitialized)
    ));
}

#[test]
fn try_init_via_env_then_returns_same_profile() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    fixture::set_env(
        "SYSTEMPROMPT_PROFILE",
        &fx.profile_path.display().to_string(),
    );

    let first = ProfileBootstrap::try_init().unwrap();
    let second = ProfileBootstrap::try_init().unwrap();

    assert!(std::ptr::eq(first, second));
}

#[test]
fn get_and_get_path_error_before_init() {
    assert!(matches!(
        ProfileBootstrap::get().unwrap_err(),
        ProfileBootstrapError::NotInitialized
    ));
    assert!(matches!(
        ProfileBootstrap::get_path().unwrap_err(),
        ProfileBootstrapError::NotInitialized
    ));
    assert!(!ProfileBootstrap::is_initialized());
}

#[test]
fn init_from_path_rejects_profile_failing_validation() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    let yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    std::fs::write(&fx.profile_path, yaml.replace("port: 8080", "port: 0")).unwrap();

    let err = ProfileBootstrap::init_from_path(&fx.profile_path).unwrap_err();

    assert!(
        err.to_string().contains("port must be greater than 0"),
        "got: {err}"
    );
    assert!(!ProfileBootstrap::is_initialized());
}
