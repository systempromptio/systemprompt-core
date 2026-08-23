//! `McpToolProvider::health_check` and `refresh_connections` over *managed*
//! (internal) servers — the branch the external-server suite in
//! `tool_provider_live` never reaches, since external servers are not managed
//! and leave the health map empty.

use std::collections::HashMap;

use systemprompt_identifiers::{Actor, ContextId, SessionId, UserId};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use systemprompt_traits::{ToolContext, ToolProvider};
use wiremock::MockServer;

use crate::harness::{
    agent_block, config_with_servers, default_tools_json, internal_server_block,
    mount_mcp_endpoint, register_internal_extension, write_services_config,
};

fn resilience() -> ResilienceSettings {
    serde_json::from_str("{}").expect("resilience defaults")
}

fn tool_context() -> ToolContext {
    let mut headers = HashMap::new();
    headers.insert("x-context-id".to_owned(), ContextId::generate().to_string());
    headers.insert("x-agent-name".to_owned(), "harness-agent".to_owned());

    let mut context = ToolContext::new(Actor::user(UserId::new("user-tph")), "token-tph");
    context.session_id = Some(SessionId::new("s-tph"));
    context.headers = headers;
    context
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

// Internal MCP servers are validated against the 5000-5999 range, so an
// ephemeral port would be rejected by config validation before any probe runs.
fn listener_in_mcp_range() -> std::net::TcpListener {
    systemprompt_test_fixtures::bind_in_range(5000..6000)
        .expect("no free port in the internal MCP range 5000-5999")
}

fn dead_port() -> u16 {
    let listener = listener_in_mcp_range();
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

async fn provider_over_internal_servers(
    agent: &str,
    blocks: &[String],
    assigned: &[&str],
) -> Option<McpToolProvider> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;

    write_services_config(
        bootstrap,
        &format!(
            "{}{}",
            agent_block(agent, assigned),
            config_with_servers(blocks)
        ),
    );

    Some(McpToolProvider::new(
        db,
        RegistryService::new(fixture_user_id()),
        &resilience(),
    ))
}

#[tokio::test]
async fn health_check_separates_reachable_managed_servers_from_dead_ones() {
    let bootstrap = ensure_test_bootstrap();
    let listener = listener_in_mcp_range();
    let mock_port = listener.local_addr().expect("addr").port();
    let mock = MockServer::builder().listener(listener).start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let up = unique("tphup");
    let down = unique("tphdown");
    register_internal_extension(bootstrap, &up);
    register_internal_extension(bootstrap, &down);

    let Some(provider) = provider_over_internal_servers(
        "tph_agent",
        &[
            internal_server_block(&up, mock_port),
            internal_server_block(&down, dead_port()),
        ],
        &[&up],
    )
    .await
    else {
        return;
    };

    let statuses = provider.health_check().await.expect("health check runs");

    assert_eq!(
        statuses.get(&up),
        Some(&true),
        "a managed server answering on its port is healthy, got {statuses:?}"
    );
    assert_eq!(
        statuses.get(&down),
        Some(&false),
        "a managed server with nothing listening is unhealthy, got {statuses:?}"
    );
}

#[tokio::test]
async fn health_check_failures_open_the_per_server_circuit_breaker() {
    let bootstrap = ensure_test_bootstrap();
    let down = unique("tphbreak");
    register_internal_extension(bootstrap, &down);

    let Some(provider) = provider_over_internal_servers(
        "tph_break",
        &[internal_server_block(&down, dead_port())],
        &[&down],
    )
    .await
    else {
        return;
    };

    for _ in 0..3 {
        let statuses = provider.health_check().await.expect("health check runs");
        assert_eq!(statuses.get(&down), Some(&false));
    }

    let context = tool_context();
    let request = systemprompt_traits::ToolCallRequest {
        tool_call_id: "call-break".to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };
    let err = provider
        .call_tool(
            &request,
            &systemprompt_identifiers::McpServerId::new(&down),
            &context,
        )
        .await
        .expect_err("a dead managed server cannot serve a tool call");
    assert!(
        err.to_string().contains(&down),
        "the failure names the server: {err}"
    );
}

#[tokio::test]
async fn refresh_connections_tolerates_a_managed_server_that_is_not_listening() {
    let bootstrap = ensure_test_bootstrap();
    let down = unique("tphrefresh");
    register_internal_extension(bootstrap, &down);

    let Some(provider) = provider_over_internal_servers(
        "tph_refresh",
        &[internal_server_block(&down, dead_port())],
        &[&down],
    )
    .await
    else {
        return;
    };

    provider
        .refresh_connections("tph_refresh")
        .await
        .expect("an unreachable managed server is logged, not fatal");
}
