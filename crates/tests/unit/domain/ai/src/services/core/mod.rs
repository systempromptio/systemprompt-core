// End-to-end tests for the AiService orchestration pipeline, driven against
// wiremock provider endpoints. AiService::new builds its clients from a
// ResolvedCatalog whose endpoint URL we override to the mock server URI, so
// the full generate / tool / stream / plan / response paths run against
// canned HTTP bodies and persist audit rows to the migrated test DB.

mod ai_service;
mod image_service;
mod provider_impl;
mod request_storage;
mod service_failure_paths;
mod stream_audit;

use std::sync::Arc;

use systemprompt_ai::{AiService, NoopToolProvider};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::services::AiConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::catalog::CatalogFixture;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, ensure_test_secrets_bootstrap, seed_user_row, seed_user_session,
    test_db_pool, unique_user_id,
};
use systemprompt_traits::{
    AiProviderResult, AiSessionProvider, CreateAiSessionParams, DynAiSessionProvider,
};

struct NoopSessionProvider;

#[async_trait::async_trait]
impl AiSessionProvider for NoopSessionProvider {
    async fn create_session(&self, _params: CreateAiSessionParams<'_>) -> AiProviderResult<()> {
        Ok(())
    }

    async fn increment_ai_usage(
        &self,
        _session_id: &systemprompt_identifiers::SessionId,
        _tokens: i32,
        _cost_microdollars: i64,
    ) -> AiProviderResult<()> {
        Ok(())
    }

    async fn find_live_session(
        &self,
        _session_id: &systemprompt_identifiers::SessionId,
    ) -> AiProviderResult<Option<systemprompt_traits::ActiveSession>> {
        Ok(None)
    }
}

pub(crate) fn noop_session_provider() -> DynAiSessionProvider {
    Arc::new(NoopSessionProvider)
}

pub(crate) const fn default_model_of(provider: &str) -> &'static str {
    match provider.as_bytes() {
        b"openai" => "gpt-4.1",
        b"gemini" => "gemini-2.5-flash",
        _ => "claude-sonnet-5",
    }
}

// The embedded default catalog with `provider`'s `api` endpoint pointed at
// the mock server, every other provider disabled, and gateway.default_model
// set to one of `provider`'s models, so the in-process default is that
// provider. The secrets bootstrap supplies placeholder keys.
pub(crate) fn fixture_with_endpoint(provider: &str, endpoint: String) -> CatalogFixture {
    let base = CatalogFixture::embedded_default();
    let others: Vec<String> = base
        .document()
        .providers
        .providers
        .keys()
        .filter(|id| id.as_str() != provider)
        .map(|id| id.as_str().to_owned())
        .collect();
    let mut fixture = base.with_provider(provider, |p| {
        p.endpoints
            .get_mut("api")
            .unwrap_or_else(|| panic!("provider '{provider}' has an api endpoint"))
            .url = endpoint;
    });
    for other in others {
        fixture = fixture.with_provider(&other, |p| p.enabled = false);
    }
    fixture.default_model(default_model_of(provider))
}

pub(crate) fn catalog_with_endpoint(provider: &str, endpoint: String) -> Arc<ResolvedCatalog> {
    Arc::new(fixture_with_endpoint(provider, endpoint).build())
}

pub(crate) fn ai_config() -> AiConfig {
    AiConfig {
        default_max_output_tokens: Some(512),
        ..AiConfig::default()
    }
}

pub(crate) async fn bootstrapped_pool() -> DbPool {
    ensure_test_bootstrap();
    ensure_test_secrets_bootstrap();
    test_db_pool().await
}

pub(crate) fn service_from(
    pool: &DbPool,
    catalog: Arc<ResolvedCatalog>,
) -> systemprompt_ai::error::Result<AiService> {
    AiService::new(
        catalog,
        &ai_config(),
        systemprompt_ai::AiServiceProviders {
            tools: Arc::new(NoopToolProvider::new()),
            sessions: noop_session_provider(),
        },
        &systemprompt_ai::repository::AiRepositories::new(pool),
    )
}

// Build an AiService whose `provider` upstream points at `endpoint`, backed by
// the real test DB so audit writes succeed.
pub(crate) fn service(pool: &DbPool, provider: &str, endpoint: String) -> AiService {
    service_from(pool, catalog_with_endpoint(provider, endpoint)).expect("AiService builds")
}

// Seed a user plus a session row and return a RequestContext carrying both,
// so the ai_requests audit FKs to `users` and `user_sessions` are satisfied
// even when the test wires a non-persisting session provider.
pub(crate) async fn seeded_context(pool: &DbPool) -> (UserId, RequestContext) {
    let user_id = unique_user_id("ai-core");
    let email = format!("{}@ai-core.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email)
        .await
        .expect("seed user");
    let session_id = SessionId::generate();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .expect("seed session");
    let context = RequestContext::new(
        session_id,
        TraceId::generate(),
        ContextId::try_new(uuid::Uuid::new_v4().to_string()).expect("valid ContextId"),
        AgentName::try_new("ai-core-test").expect("valid AgentName"),
        Actor::user(user_id.clone()),
    );
    (user_id, context)
}
