//! Provider-trait impls on `RegistryService` over a POPULATED registry: the
//! Some/success arms that the config-error smoke tests never reach.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::RegistryService;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{McpRegistry, McpToolProvider};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_user_id};
use systemprompt_traits::McpRegistryProvider;
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, config_with_servers, default_tools_json, external_server_block,
    mount_mcp_endpoint, write_services_config,
};

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-rtp"),
        TraceId::new("t-rtp"),
        ContextId::generate(),
        AgentName::new("agent-rtp"),
    )
}

async fn populated_registry() -> (RegistryService, String, MockServer) {
    let bootstrap = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let name = format!("rtp_{}", uuid::Uuid::new_v4().simple());
    write_services_config(
        bootstrap,
        &config_with_servers(&[external_server_block(&ExternalServerSpec {
            name: &name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        })]),
    );
    (RegistryService::new(fixture_user_id()), name, mock)
}

#[tokio::test]
async fn registry_find_server_returns_state_for_known_server() {
    let (registry, name, _mock) = populated_registry().await;

    let state = McpRegistry::find_server(&registry, &name)
        .await
        .expect("registry reachable")
        .expect("server known");
    assert_eq!(state.name, name);
    assert_eq!(state.status, "unknown");

    let servers = McpRegistry::list_servers(&registry)
        .await
        .expect("list servers");
    assert!(servers.contains(&name));
    assert!(
        McpRegistry::server_exists(&registry, &name)
            .await
            .expect("exists check")
    );
}

#[tokio::test]
async fn tool_provider_trait_lists_tools_from_scripted_server() {
    let (registry, name, _mock) = populated_registry().await;

    let tools = McpToolProvider::list_tools(&registry, &name, &ctx())
        .await
        .expect("tools listed");
    assert!(tools.iter().any(|t| t.name == "echo"));

    let by_server = registry
        .load_tools_for_servers(&[name.clone()], &ctx())
        .await
        .expect("tools loaded");
    assert_eq!(by_server.get(&name).map(Vec::len), Some(2));
}

#[tokio::test]
async fn registry_provider_reports_server_info_and_enabled_set() {
    let (registry, name, _mock) = populated_registry().await;

    let info = McpRegistryProvider::get_server(&registry, &name)
        .await
        .expect("server info");
    assert_eq!(info.name, name);
    assert!(info.enabled);
    assert!(!info.oauth.required);
    assert_eq!(info.oauth.audience, "mcp");

    let enabled = registry
        .list_enabled_servers()
        .await
        .expect("enabled servers");
    assert!(enabled.iter().any(|s| s.name == name));
}
