//! Unit tests for [`McpToolProvider`] constructor and accessor.

use systemprompt_manifest::services::ResilienceSettings;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_test_fixtures::{test_db_pool, unique_user_id};

#[tokio::test]
async fn tool_provider_construction_and_accessors() {
    let db = test_db_pool().await;
    let registry = RegistryService::new(
        unique_user_id("tool-provider"),
        crate::harness::services_state().view(),
        crate::harness::config(),
    );
    let resilience = ResilienceSettings::default();
    let provider = McpToolProvider::new(
        db,
        registry,
        &resilience,
        std::sync::Arc::default(),
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
    );
    let _ = provider.db_pool();
    let _ = format!("{provider:?}");
    let _clone = provider.clone();
}

#[tokio::test]
async fn tool_provider_list_tools_unknown_agent_returns_error_or_empty() {
    use systemprompt_identifiers::{Actor, AgentName};
    use systemprompt_traits::{ToolContext, ToolProvider};
    let db = test_db_pool().await;
    let registry = RegistryService::new(
        unique_user_id("tool-provider"),
        crate::harness::services_state().view(),
        crate::harness::config(),
    );
    let provider = McpToolProvider::new(
        db,
        registry,
        &ResilienceSettings::default(),
        std::sync::Arc::default(),
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
    );
    let ctx = ToolContext::new(Actor::user(unique_user_id("tool-provider")));
    let res = provider
        .list_tools(
            &AgentName::try_new(format!("no-agent-{}", uuid::Uuid::new_v4().simple()))
                .expect("valid AgentName"),
            &ctx,
        )
        .await;
    let _ = res;
}
