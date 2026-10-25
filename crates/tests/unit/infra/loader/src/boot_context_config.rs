use systemprompt_manifest::Config;
use systemprompt_models::errors::GlobalConfigError;
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn a_boot_context_answers_not_initialised_until_config_is_built() {
    let fixture = ensure_test_bootstrap();
    let fresh = systemprompt_loader::BootContext::new(
        fixture.boot.loaded_profile().clone(),
        fixture.boot.secrets().clone(),
    );

    assert!(!fresh.is_config_initialized());
    assert!(matches!(
        fresh.config(),
        Err(GlobalConfigError::NotInitialized)
    ));
}

#[test]
fn try_init_config_builds_once_and_keeps_the_first_value() {
    let fixture = ensure_test_bootstrap();
    let fresh = systemprompt_loader::BootContext::new(
        fixture.boot.loaded_profile().clone(),
        fixture.boot.secrets().clone(),
    );

    let first = fresh.try_init_config(None).expect("config builds").clone();
    let second = fresh.try_init_config(None).expect("second call is a no-op");

    assert!(std::sync::Arc::ptr_eq(&first, second));
    assert!(fresh.is_config_initialized());
    assert_eq!(first.database_url, fixture.boot.secrets().database_url());
}

#[test]
fn install_config_refuses_a_second_value() {
    let fixture = ensure_test_bootstrap();
    let fresh = systemprompt_loader::BootContext::new(
        fixture.boot.loaded_profile().clone(),
        fixture.boot.secrets().clone(),
    );
    let config: Config = (**fixture.boot.config().expect("fixture config")).clone();

    fresh.install_config(config.clone()).expect("first install");

    assert!(fresh.install_config(config).is_err());
}
