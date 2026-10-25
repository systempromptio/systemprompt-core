use systemprompt_config::{ConfigError, LoadedProfile, ProfileBootstrapError};

use crate::fixture;

#[test]
fn from_env_reads_path_from_systemprompt_profile_env() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    fixture::set_env(
        "SYSTEMPROMPT_PROFILE",
        &fx.profile_path.display().to_string(),
    );

    let profile = LoadedProfile::from_env().unwrap();

    assert_eq!(profile.name, "config_fixture");
    assert_eq!(profile.path(), fx.profile_path.as_path());
}

#[test]
fn from_env_errors_when_env_var_unset() {
    fixture::remove_env("SYSTEMPROMPT_PROFILE");

    let err = LoadedProfile::from_env().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Profile(ProfileBootstrapError::PathNotSet)
    ));
}

#[test]
fn from_env_errors_when_profile_file_missing() {
    fixture::set_env("SYSTEMPROMPT_PROFILE", "/nonexistent/profile.yaml");

    let err = LoadedProfile::from_env().unwrap_err();

    assert!(matches!(err, ConfigError::ProfileParse(_)), "got: {err:?}");
}

#[test]
fn from_path_loads_independent_values() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);

    let first = LoadedProfile::from_path(&fx.profile_path).unwrap();
    let second = LoadedProfile::from_path(&fx.profile_path).unwrap();

    assert!(!std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(first.name, second.name);
}

#[test]
fn from_path_rejects_profile_failing_validation() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    let yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    std::fs::write(&fx.profile_path, yaml.replace("port: 8080", "port: 0")).unwrap();

    let err = LoadedProfile::from_path(&fx.profile_path).unwrap_err();

    assert!(
        err.to_string().contains("port must be greater than 0"),
        "got: {err}"
    );
}
