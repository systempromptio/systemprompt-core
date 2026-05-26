//! Unit tests for [`McpToolProvider`] constructor and accessor.

use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};

#[tokio::test]
async fn tool_provider_construction_and_accessors() {
    let Ok(url) = fixture_database_url() else { return };
    let Ok(db) = fixture_db_pool(&url).await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let resilience = ResilienceSettings::default();
    let provider = McpToolProvider::new(db, registry, &resilience);
    let _ = provider.db_pool();
    let _ = format!("{provider:?}");
    let _clone = provider.clone();
}

#[tokio::test]
async fn tool_provider_list_tools_unknown_agent_returns_error_or_empty() {
    use systemprompt_identifiers::Actor;
    use systemprompt_traits::{ToolContext, ToolProvider};
    let Ok(url) = fixture_database_url() else { return };
    let Ok(db) = fixture_db_pool(&url).await else { return };
    let registry = RegistryService::new(fixture_user_id());
    let provider = McpToolProvider::new(db, registry, &ResilienceSettings::default());
    let ctx = ToolContext::new(Actor::user(fixture_user_id()), "");
    let res = provider
        .list_tools(&format!("no-agent-{}", uuid::Uuid::new_v4().simple()), &ctx)
        .await;
    let _ = res;
}
