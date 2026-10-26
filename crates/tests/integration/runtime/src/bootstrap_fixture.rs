//! Smoke test for the process-wide bootstrap fixture.
//!
//! Verifies the boot context (profile, secrets, config) and the files config
//! derived from it resolve without panicking and report consistent values
//! across repeat calls.

use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn bootstrap_resolves_profile_secrets_config_and_files_config() {
    let env = ensure_test_bootstrap();

    assert!(env.profile_path.exists(), "profile.yaml should exist");
    assert!(env.system_path.exists(), "system dir should exist");
    assert!(env.services_path.exists(), "services dir should exist");

    assert_eq!(env.boot.profile().name, "test");

    let secrets = env.boot.secrets().get();
    assert!(!secrets.database_url.is_empty());

    let cfg = env.boot_config();
    assert_eq!(cfg.system_admin_username, "testadmin");
    assert_eq!(cfg.sitename, "testsite");

    assert!(
        systemprompt_files::FilesConfig::from_profile(env.boot.profile(), &env.app_paths).is_ok()
    );
}

#[test]
fn bootstrap_is_idempotent() {
    let env1 = ensure_test_bootstrap();
    let env2 = ensure_test_bootstrap();
    assert_eq!(env1.profile_path, env2.profile_path);
    assert_eq!(env1.database_url, env2.database_url);
}
