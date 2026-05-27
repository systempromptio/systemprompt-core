//! Smoke test for the process-wide bootstrap fixture.
//!
//! Verifies all four global singletons (Profile, Secrets, Config,
//! FilesConfig) initialise without panicking and report consistent
//! values across repeat calls.

use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn bootstrap_initialises_all_globals() {
    let env = ensure_test_bootstrap();

    assert!(env.profile_path.exists(), "profile.yaml should exist");
    assert!(env.system_path.exists(), "system dir should exist");
    assert!(env.services_path.exists(), "services dir should exist");

    let profile = systemprompt_config::ProfileBootstrap::get().expect("profile initialised");
    assert_eq!(profile.name, "test");

    let secrets = systemprompt_config::SecretsBootstrap::get().expect("secrets initialised");
    assert!(!secrets.database_url.is_empty());

    let cfg = systemprompt_models::Config::get().expect("config initialised");
    assert_eq!(cfg.system_admin_username, "testadmin");
    assert_eq!(cfg.sitename, "testsite");

    assert!(systemprompt_files::FilesConfig::get_optional().is_some());
}

#[test]
fn bootstrap_is_idempotent() {
    let env1 = ensure_test_bootstrap();
    let env2 = ensure_test_bootstrap();
    assert_eq!(env1.profile_path, env2.profile_path);
    assert_eq!(env1.database_url, env2.database_url);
}
