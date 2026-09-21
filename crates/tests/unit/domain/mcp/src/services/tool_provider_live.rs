//! Drives `McpToolProvider` through the `ToolProvider` trait against a
//! scripted MCP endpoint resolved from the bootstrap services config.

use std::collections::HashMap;

use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpServerId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, fixture_user_id};
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};
use wiremock::MockServer;

use crate::harness::{
    ExternalServerSpec, agent_block, bootstrap_with_services, config_with_servers,
    default_tools_json, external_server_block, mount_mcp_endpoint,
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

async fn setup_or_skip(agent: &str) -> Option<(McpToolProvider, McpServerId, MockServer)> {
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
    let _bootstrap = bootstrap_with_services(&yaml);

    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), &resilience());
    Some((
        provider,
        McpServerId::try_new(&server_name).expect("valid McpServerId"),
        mock,
    ))
}

#[tokio::test]
async fn list_tools_resolves_agent_servers() {
    let Some((provider, _server, _mock)) = setup_or_skip("tp_agent_list").await else {
        return;
    };

    let tools = provider
        .list_tools(
            &AgentName::try_new("tp_agent_list").expect("valid AgentName"),
            &tool_context(),
        )
        .await
        .expect("tools listed");
    assert!(tools.is_complete());
    assert_eq!(tools.tools.len(), 2);
    assert!(tools.tools.iter().any(|t| t.name == "echo"));
    let _ = provider.db_pool();
}

#[tokio::test]
async fn list_tools_unknown_agent_is_configuration_error() {
    let Some((provider, _server, _mock)) = setup_or_skip("tp_agent_missing").await else {
        return;
    };

    let err = provider
        .list_tools(
            &AgentName::try_new("no-such-agent").expect("valid AgentName"),
            &tool_context(),
        )
        .await
        .expect_err("unknown agent rejected");
    assert!(err.to_string().contains("Failed to load agent config"));
}

#[tokio::test]
async fn call_tool_executes_through_resilience_guard() {
    let Some((provider, server, _mock)) = setup_or_skip("tp_agent_call").await else {
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
    let Some((provider, _server, _mock)) = setup_or_skip("tp_agent_badsrv").await else {
        return;
    };

    let request = ToolCallRequest {
        tool_call_id: "call-2".to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };

    let err = provider
        .call_tool(
            &request,
            &McpServerId::try_new("no-such-server").expect("valid McpServerId"),
            &tool_context(),
        )
        .await
        .expect_err("unknown server rejected");
    assert!(err.to_string().contains("Failed to resolve MCP server"));
}

#[tokio::test]
async fn call_tool_requires_context_headers() {
    let Some((provider, server, _mock)) = setup_or_skip("tp_agent_hdrs").await else {
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
    let Some((provider, _server, _mock)) = setup_or_skip("tp_agent_refresh").await else {
        return;
    };

    provider
        .refresh_connections(&AgentName::try_new("tp_agent_refresh").expect("valid AgentName"))
        .await
        .expect("refresh validates");
}

#[tokio::test]
async fn health_check_reports_no_managed_servers() {
    let Some((provider, _server, _mock)) = setup_or_skip("tp_agent_health").await else {
        return;
    };

    let statuses = provider.health_check().await.expect("health check runs");
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn list_tools_tolerates_unreachable_server() {
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
    let _bootstrap = bootstrap_with_services(&yaml);

    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), &resilience());
    let tools = provider
        .list_tools(
            &AgentName::try_new("tp_agent_down").expect("valid AgentName"),
            &tool_context(),
        )
        .await
        .expect("an unreachable server is reported, not silently skipped");
    assert!(tools.tools.is_empty());
    assert_eq!(
        tools.failed_servers.len(),
        1,
        "the failed server is named in the inventory: {tools:?}"
    );
}

#[tokio::test]
async fn list_tools_keeps_healthy_server_inventory_when_another_assigned_server_is_unreachable() {
    let url = fixture_database_url().expect("MCP tool-provider database URL");
    let db = fixture_db_pool(&url)
        .await
        .expect("MCP tool-provider database");
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let healthy = format!("tp_up_{}", uuid::Uuid::new_v4().simple());
    let unavailable = format!("tp_down_{}", uuid::Uuid::new_v4().simple());
    let yaml = format!(
        "{}{}",
        agent_block("tp_agent_partial", &[&healthy, &unavailable]),
        config_with_servers(&[
            external_server_block(&ExternalServerSpec {
                name: &healthy,
                endpoint: &format!("{}/mcp", mock.uri()),
                oauth_required: false,
                enabled: true,
            }),
            external_server_block(&ExternalServerSpec {
                name: &unavailable,
                endpoint: "http://127.0.0.1:1/mcp",
                oauth_required: false,
                enabled: true,
            }),
        ])
    );
    let _bootstrap = bootstrap_with_services(&yaml);
    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), &resilience());

    let inventory = provider
        .list_tools(
            &AgentName::try_new("tp_agent_partial").expect("valid AgentName"),
            &tool_context(),
        )
        .await
        .expect("one server failure is represented in the inventory");
    assert_eq!(
        inventory
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec!["echo", "shout"]
    );
    assert!(
        inventory
            .tools
            .iter()
            .all(|tool| tool.service_id.as_str() == healthy)
    );
    assert_eq!(
        inventory.tools[0].input_schema,
        Some(serde_json::json!({"type": "object", "properties": {"message": {"type": "string"}}}))
    );
    assert_eq!(
        inventory.tools[1].output_schema,
        Some(serde_json::json!({"type": "object"}))
    );
    assert_eq!(inventory.failed_servers.len(), 1);
    assert_eq!(inventory.failed_servers[0].server.as_str(), unavailable);
    assert!(
        inventory.failed_servers[0].message.contains("127.0.0.1:1"),
        "failed server retains its transport endpoint diagnosis: {:?}",
        inventory.failed_servers[0]
    );
}
