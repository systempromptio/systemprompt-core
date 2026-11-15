use std::sync::Arc;

use systemprompt_extension::{FactoryError, Resolved, SlotError};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_oauth_issuance::contract::SsoBuilder;
use systemprompt_oauth_issuance::sso::oidc::CONFIG_FILE;
use systemprompt_oauth_issuance::sso::{SsoDriver, SsoHookContext, SsoProviderError, oidc};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_config, test_db_pool};
use systemprompt_traits::sso::SsoProvider;

use super::stores::{CountingSessions, MemoryUsers};

async fn discover_with_oidc(oidc_yaml: &str) -> Result<SsoDriver, SsoProviderError> {
    ensure_test_bootstrap();
    let db = test_db_pool().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join(CONFIG_FILE);
    std::fs::create_dir_all(file.parent().expect("config dir")).expect("mkdir");
    std::fs::write(&file, oidc_yaml).expect("write oidc.yaml");
    let mut services_root = systemprompt_test_fixtures::test_boot_active_services_root();
    services_root.path = dir.path().to_path_buf();
    let config = Arc::new(fixture_config("postgres://unused/unused"));
    let service = SessionCreationService::new(
        Arc::new(CountingSessions::default()) as Arc<dyn systemprompt_traits::SessionProvider>,
        Arc::new(MemoryUsers::default()) as Arc<dyn systemprompt_traits::UserProvider>,
        systemprompt_test_fixtures::test_token_authority(
            systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
        ),
        Arc::clone(&config),
    );
    let hook = SsoHookContext {
        db: Arc::clone(&db),
        config: Arc::clone(&config),
        services: systemprompt_test_fixtures::fixture_services_view(),
        services_root,
        secrets: systemprompt_test_fixtures::fixture_secret_store(),
    };
    let mut sso = SsoBuilder::default();
    sso.provider(
        move |_: &Resolved<'_>| -> Result<Option<Arc<dyn SsoProvider>>, FactoryError> {
            let built = oidc::build(&hook).map_err(|source| {
                FactoryError::Failed(Box::new(SsoProviderError::config("oidc", source)))
            })?;
            Ok(built.map(|provider| Arc::new(provider) as Arc<dyn SsoProvider>))
        },
    );
    let built = match super::build_sso(sso) {
        Ok(built) => built,
        Err(SlotError::Entry { source, .. }) => {
            return Err(*source
                .downcast::<SsoProviderError>()
                .expect("the slot carries the provider's own error"));
        },
        Err(other) => panic!("unexpected slot error: {other:?}"),
    };
    Ok(SsoDriver::new(
        &built,
        &db,
        config,
        systemprompt_test_fixtures::oauth_fixture_repository(&db),
        Arc::new(service),
    ))
}

#[tokio::test]
async fn an_enabled_oidc_config_without_its_client_secret_fails_discovery() {
    let err = discover_with_oidc(
        "enabled: true\nissuer: https://idp.example.com/realms/test\nclient_id: core\nclient_secret_key: oidc_secret_absent_from_fixture_store\n",
    )
    .await
    .expect_err("an enabled but unusable provider must stop the boot");

    match err {
        SsoProviderError::Config { provider_id, .. } => assert_eq!(provider_id, "oidc"),
        other => panic!("expected a config error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_disabled_oidc_config_registers_no_provider() {
    let driver = discover_with_oidc(
        "enabled: false\nissuer: https://idp.example.com/realms/test\nclient_id: core\n",
    )
    .await
    .expect("a disabled provider is simply off");

    assert!(
        driver.provider("oidc").is_none(),
        "{:?}",
        driver.provider_ids()
    );
}
