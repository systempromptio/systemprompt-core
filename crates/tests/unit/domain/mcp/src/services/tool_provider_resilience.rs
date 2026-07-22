//! Drives `McpToolProvider::call_tool` into the resilience guard's timeout,
//! circuit-open, bulkhead-full, and inner-error arms.

use std::collections::HashMap;
use std::time::Duration;

use systemprompt_identifiers::{Actor, ContextId, McpServerId, SessionId, TraceId, UserId};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_mcp::services::tool_provider::McpToolProvider;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use systemprompt_traits::{ToolCallRequest, ToolContext, ToolProvider};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::harness::{
    ExternalServerSpec, agent_block, config_with_servers, default_tools_json,
    external_server_block, mount_mcp_endpoint, write_services_config,
};

fn settings(overrides: serde_json::Value) -> ResilienceSettings {
    serde_json::from_value(overrides).expect("resilience settings")
}

fn tool_context() -> ToolContext {
    let mut headers = HashMap::new();
    headers.insert("x-context-id".to_owned(), ContextId::generate().to_string());
    headers.insert("x-agent-name".to_owned(), "resilience-agent".to_owned());
    headers.insert("x-user-id".to_owned(), "user-res".to_owned());
    headers.insert("x-task-id".to_owned(), "task-res".to_owned());

    let mut context = ToolContext::new(Actor::user(UserId::new("user-res")), "token-res");
    context.session_id = Some(SessionId::new("s-res"));
    context.trace_id = Some(TraceId::new("t-res"));
    context.headers = headers;
    context
}

fn call_request(id: &str) -> ToolCallRequest {
    ToolCallRequest {
        tool_call_id: id.to_owned(),
        name: "echo".to_owned(),
        arguments: serde_json::json!({"message": "hi"}),
    }
}

async fn provider_for_endpoint(
    agent: &str,
    endpoint: &str,
    resilience: &ResilienceSettings,
) -> Option<(McpToolProvider, McpServerId)> {
    let bootstrap = ensure_test_bootstrap();
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;

    let server_name = format!("res_{}", uuid::Uuid::new_v4().simple());
    let yaml = format!(
        "{}{}",
        agent_block(agent, &[&server_name]),
        config_with_servers(&[external_server_block(&ExternalServerSpec {
            name: &server_name,
            endpoint,
            oauth_required: false,
            enabled: true,
        })])
    );
    write_services_config(bootstrap, &yaml);

    let provider = McpToolProvider::new(db, RegistryService::new(fixture_user_id()), resilience);
    Some((provider, McpServerId::new(&server_name)))
}

async fn mount_delayed_tool_call(mock: &MockServer, delay: Duration) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_delay(delay)
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "slow output"}],
                        "isError": false
                    }
                })),
        )
        .mount(mock)
        .await;
}

#[tokio::test]
async fn call_tool_maps_per_attempt_timeout() {
    let mock = MockServer::start().await;
    mount_delayed_tool_call(&mock, Duration::from_secs(10)).await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let resilience = settings(serde_json::json!({
        "request_timeout_ms": 500,
        "retry_attempts": 1
    }));
    let Some((provider, server)) = provider_for_endpoint(
        "res_agent_timeout",
        &format!("{}/mcp", mock.uri()),
        &resilience,
    )
    .await
    else {
        return;
    };

    let err = provider
        .call_tool(&call_request("call-timeout"), &server, &tool_context())
        .await
        .expect_err("slow server must time out");
    let msg = err.to_string();
    assert!(msg.contains("timed out after"), "got: {msg}");
    assert!(msg.contains(server.as_str()), "got: {msg}");
}

#[tokio::test]
async fn call_tool_maps_inner_error_then_circuit_open() {
    let resilience = settings(serde_json::json!({
        "retry_attempts": 1,
        "breaker_failure_threshold": 1,
        "breaker_open_cooldown_ms": 60_000
    }));
    let Some((provider, server)) =
        provider_for_endpoint("res_agent_circuit", "http://127.0.0.1:1/mcp", &resilience).await
    else {
        return;
    };

    let inner = provider
        .call_tool(&call_request("call-inner"), &server, &tool_context())
        .await
        .expect_err("unreachable server must fail");
    assert!(
        !inner.to_string().contains("circuit breaker open"),
        "first failure surfaces the inner error, got: {inner}"
    );

    let open = provider
        .call_tool(&call_request("call-open"), &server, &tool_context())
        .await
        .expect_err("tripped breaker must fail fast");
    let msg = open.to_string();
    assert!(msg.contains("circuit breaker open"), "got: {msg}");
    assert!(msg.contains(server.as_str()), "got: {msg}");
}

#[tokio::test]
async fn call_tool_maps_bulkhead_full_under_concurrency() {
    let mock = MockServer::start().await;
    mount_delayed_tool_call(&mock, Duration::from_secs(20)).await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let resilience = settings(serde_json::json!({
        "request_timeout_ms": 30_000,
        "retry_attempts": 1,
        "max_concurrent": 1
    }));
    let Some((provider, server)) = provider_for_endpoint(
        "res_agent_bulkhead",
        &format!("{}/mcp", mock.uri()),
        &resilience,
    )
    .await
    else {
        return;
    };

    let slow_provider = provider.clone();
    let slow_server = server.clone();
    let slow = tokio::spawn(async move {
        slow_provider
            .call_tool(&call_request("call-slow"), &slow_server, &tool_context())
            .await
    });

    let mut permit_held = false;
    for _ in 0..2000 {
        let saw_tool_call = mock.received_requests().await.is_some_and(|requests| {
            requests
                .iter()
                .any(|r| String::from_utf8_lossy(&r.body).contains("tools/call"))
        });
        if saw_tool_call {
            permit_held = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(permit_held, "spawned call never reached the MCP endpoint");

    let rejection = provider
        .call_tool(&call_request("call-rejected"), &server, &tool_context())
        .await;
    slow.abort();

    let err = rejection.expect_err("saturated bulkhead must reject the concurrent call");
    let msg = err.to_string();
    assert!(msg.contains("concurrency limit reached"), "got: {msg}");
    assert!(msg.contains(server.as_str()), "got: {msg}");
}
