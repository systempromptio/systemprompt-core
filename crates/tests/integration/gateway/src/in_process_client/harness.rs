use std::sync::Arc;

use systemprompt_catalog_fixtures::{CatalogFixture, platform};
use systemprompt_database::DbPool;
use systemprompt_gateway::GatewayRepositories;
use systemprompt_manifest::services::catalog::{
    EndpointConfig, ModelEntry, ProviderConfig, ResolvedCatalog,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

const PROVIDER: &str = "anthropic";
pub(super) const MODEL: &str = "claude-test";

pub(super) fn repos(db: &DbPool) -> GatewayRepositories {
    GatewayRepositories::new(
        db,
        systemprompt_gateway::audit::journal::GatewayJournal::open(
            systemprompt_test_fixtures::ensure_test_bootstrap()
                .app_paths
                .storage()
                .data(),
            systemprompt_test_fixtures::ensure_test_secrets_bootstrap()
                .encryption_master_key()
                .expect("encryption master key"),
        )
        .expect("gateway journal opens"),
        Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db),
        )),
        systemprompt_gateway::GatewayHandles {
            background: systemprompt_traits::BackgroundTasks::new(),
            cache_epochs: systemprompt_traits::CacheEpochs::new(),
            logging: systemprompt_test_fixtures::fixture_logging_runtime(),
            services: systemprompt_test_fixtures::fixture_services_view(),
            subject_providers: systemprompt_test_fixtures::fixture_subject_providers(db),
            at_rest: systemprompt_security::at_rest::AtRestCipher::from_secrets(
                &systemprompt_test_fixtures::ensure_test_secrets_bootstrap().get(),
            )
            .expect("at-rest cipher"),
            unapplied_plane: systemprompt_manifest::profile::UnappliedPlaneMode::default(),
        },
    )
}

pub(super) fn catalog(endpoint: &str) -> ResolvedCatalog {
    CatalogFixture::new()
        .credential(PROVIDER, platform(PROVIDER))
        .provider(PROVIDER, ProviderConfig::new(PROVIDER))
        .endpoint(
            PROVIDER,
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, endpoint),
        )
        .model(MODEL, ModelEntry::new(vec![ClientFamily::Claude]))
        .deploy(MODEL, [format!("{PROVIDER}/api").as_str()])
        .build()
}
