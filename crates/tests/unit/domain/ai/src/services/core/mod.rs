// End-to-end tests for the AiService orchestration pipeline, driven against
// wiremock provider endpoints. AiService::new builds its providers from a
// ProviderRegistry whose endpoint we override to the mock server URI, so the
// full generate / tool / stream / plan / response paths run against canned
// HTTP bodies and persist audit rows to the migrated test DB.

mod ai_service;
mod image_service;

use std::sync::Arc;

use systemprompt_ai::{AiService, NoopToolProvider};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::profile::ProviderRegistry;
use systemprompt_models::services::{AiConfig, AiProviderConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, ensure_test_secrets_bootstrap, fixture_database_url, fixture_db_pool,
    seed_user_row, unique_user_id,
};

// A registry seeded from the embedded catalog with the chosen provider's
// endpoint rewritten to the mock server. The endpoint field is public, so the
// override is a direct assignment.
pub(crate) fn registry_with_endpoint(provider: &str, endpoint: String) -> ProviderRegistry {
    let mut registry = ProviderRegistry::default_seed().expect("default catalog parses");
    let entry = registry
        .providers
        .iter_mut()
        .find(|p| p.name.as_str() == provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in default catalog"));
    entry.endpoint = endpoint;
    registry
}

// A single-provider AI policy whose default_provider matches and whose entry is
// enabled. The api_key secret is absent in tests; build_providers keeps the
// provider enabled with an empty key (the mock endpoint needs no auth).
pub(crate) fn ai_config(provider: &str) -> AiConfig {
    let mut providers = std::collections::HashMap::new();
    providers.insert(
        provider.to_owned(),
        AiProviderConfig {
            enabled: true,
            ..AiProviderConfig::default()
        },
    );
    AiConfig {
        default_provider: provider.to_owned(),
        default_max_output_tokens: Some(512),
        providers,
        ..AiConfig::default()
    }
}

pub(crate) async fn pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    ensure_test_secrets_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(pool)
}

// Build an AiService whose `provider` upstream points at `endpoint`, backed by
// the real test DB so audit writes succeed.
pub(crate) fn service(pool: &DbPool, provider: &str, endpoint: String) -> AiService {
    let registry = registry_with_endpoint(provider, endpoint);
    let config = ai_config(provider);
    AiService::new(
        pool,
        &registry,
        &config,
        Arc::new(NoopToolProvider::new()),
        None,
    )
    .expect("AiService builds")
}

// Seed a user and return a RequestContext whose actor carries that user id so
// the ai_requests audit FK to `users` is satisfied.
pub(crate) async fn seeded_context(pool: &DbPool) -> (UserId, RequestContext) {
    let user_id = unique_user_id("ai-core");
    let email = format!("{}@ai-core.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email)
        .await
        .expect("seed user");
    let context = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::new(uuid::Uuid::new_v4().to_string()),
        AgentName::new("ai-core-test"),
    )
    .with_actor(Actor::user(user_id.clone()));
    (user_id, context)
}
