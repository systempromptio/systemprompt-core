//! Drives `McpToolProvider` through the `ToolProvider` trait against a
//! scripted MCP endpoint resolved from the bootstrap services config.

use std::collections::HashMap;

use systemprompt_identifiers::{Actor, ContextId, McpServerId, SessionId, TraceId, UserId};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, agent_block, config_with_servers, default_tools_json,
    external_server_block, mount_mcp_endpoint, write_services_config,
};

fn resilience() -> ResilienceSettings {
    serde_json::from_str("{}").expect("resilience defaults")
}

fn tool_context() -> ToolContext {
    let mut headers = HashMap::new();
    headers.insert("x-context-id".to_owned(), ContextId::generate().to_string());
    headers.insert("x-agent-name".to_owned(), "harness-agent".to_owned());
    headers.insert("x-user-id".to_owned(), "user-tp".to_owned());
    headers.insert("x-task-id".to_owned(), "task-tp".to_owned());

    let mut context = ToolContext::new(Actor::user(UserId::new("user-tp")), "token-tp");
    context.session_id = Some(SessionId::new("s-tp"));
    context.trace_id = Some(TraceId::new("t-tp"));
    context.headers = headers;
    context
}

async fn setup(agent: &str) -> Option<(McpToolProvider, McpServerId, MockServer)> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;

    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let server_name = format!("tp_{}", uuid::Uuid::new_v4().simple());
    let yaml = format!(
        "{}{}",
        agent_block(agent, &[&server_name]),
        config_with_servers(&[external_server_block(&ExternalServerSpec {
            name: &server_name,
            endpoint: &format!("{}/mcp", mock.uri()),
            oauth_required: false,
            enabled: true,
        })])
    );
    write_services_config(bootstrap, &yaml);

    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), &resilience());
    Some((provider, McpServerId::new(&server_name), mock))
}

#[tokio::test]
async fn list_tools_resolves_agent_servers() {
    let Some((provider, _server, _mock)) = setup("tp_agent_list").await else {
        return;
    };

    let tools = provider
        .list_tools("tp_agent_list", &tool_context())
        .await
        .expect("tools listed");
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().any(|t| t.name == "echo"));
    let _ = provider.db_pool();
}

#[tokio::test]
async fn list_tools_unknown_agent_is_configuration_error() {
    let Some((provider, _server, _mock)) = setup("tp_agent_missing").await else {
        return;
    };

    let err = provider
        .list_tools("no-such-agent", &tool_context())
        .await
        .expect_err("unknown agent rejected");
    assert!(err.to_string().contains("Failed to load agent config"));
}

#[tokio::test]
async fn call_tool_executes_through_resilience_guard() {
    let Some((provider, server, _mock)) = setup("tp_agent_call").await else {
        return;
    };

    let request = ToolCallRequest {
        tool_call_id: "call-1".to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({"message": "hi"}),
    };

    let result = provider
        .call_tool(&request, &server, &tool_context())
        .await
        .expect("tool call succeeds");
    assert_eq!(result.is_error, Some(false));

    let repeat = provider
        .call_tool(&request, &server, &tool_context())
        .await
        .expect("guard is reused");
    assert_eq!(repeat.is_error, Some(false));
}

#[tokio::test]
async fn call_tool_unknown_server_is_configuration_error() {
    let Some((provider, _server, _mock)) = setup("tp_agent_badsrv").await else {
        return;
    };

    let request = ToolCallRequest {
        tool_call_id: "call-2".to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };

    let err = provider
        .call_tool(&request, &McpServerId::new("no-such-server"), &tool_context())
        .await
        .expect_err("unknown server rejected");
    assert!(err.to_string().contains("Failed to resolve MCP server"));
}

#[tokio::test]
async fn call_tool_requires_context_headers() {
    let Some((provider, server, _mock)) = setup("tp_agent_hdrs").await else {
        return;
    };

    let request = ToolCallRequest {
        tool_call_id: "call-3".to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };

    let bare = ToolContext::new(Actor::user(UserId::new("user-bare")), "token");
    let err = provider
        .call_tool(&request, &server, &bare)
        .await
        .expect_err("missing headers rejected");
    assert!(err.to_string().contains("x-context-id"));

    let mut only_context = bare.clone();
    only_context
        .headers
        .insert("x-context-id".to_owned(), ContextId::generate().to_string());
    let err = provider
        .call_tool(&request, &server, &only_context)
        .await
        .expect_err("missing agent name rejected");
    assert!(err.to_string().contains("x-agent-name"));
}

#[tokio::test]
async fn refresh_connections_validates_reachable_server() {
    let Some((provider, _server, _mock)) = setup("tp_agent_refresh").await else {
        return;
    };

    provider
        .refresh_connections("tp_agent_refresh")
        .await
        .expect("refresh validates");
}

#[tokio::test]
async fn health_check_reports_no_managed_servers() {
    let Some((provider, _server, _mock)) = setup("tp_agent_health").await else {
        return;
    };

    let statuses = provider.health_check().await.expect("health check runs");
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn list_tools_tolerates_unreachable_server() {
    let bootstrap = ensure_test_bootstrap();
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };

    let server_name = format!("tp_down_{}", uuid::Uuid::new_v4().simple());
    let yaml = format!(
        "{}{}",
        agent_block("tp_agent_down", &[&server_name]),
        config_with_servers(&[external_server_block(&ExternalServerSpec {
            name: &server_name,
            endpoint: "http://127.0.0.1:1/mcp",
            oauth_required: false,
            enabled: true,
        })])
    );
    write_services_config(bootstrap, &yaml);

    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), &resilience());
    let tools = provider
        .list_tools("tp_agent_down", &tool_context())
        .await
        .expect("unreachable server is skipped");
    assert!(tools.is_empty());
}
