use systemprompt_mcp::services::monitoring::health::HealthStatus;
use systemprompt_mcp::services::monitoring::status::{
    McpServiceStatus, display_service_status, get_all_service_status,
};
use systemprompt_models::services::ServiceStatus;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::harness::{default_tools_json, external_mcp_config, mount_mcp_endpoint};

fn entry<'a>(statuses: &'a [McpServiceStatus], name: &str) -> Option<&'a McpServiceStatus> {
    statuses.iter().find(|s| s.name == name)
}

#[tokio::test]
async fn live_endpoint_reports_running_with_tool_count() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let config = external_mcp_config("status-live", &format!("{}/mcp", mock.uri()));

    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = entry(&statuses, "status-live").expect("entry present");
    assert_eq!(status.observed_state(), ServiceStatus::Running);
    assert_eq!(status.health, HealthStatus::Healthy);
    assert_eq!(status.tools_count, Some(2));
    assert!(status.latency_ms.is_some());
    assert!(!status.auth_required);

    display_service_status(&statuses);
}

#[tokio::test]
async fn dead_endpoint_reports_stopped() {
    let config = external_mcp_config("status-dead", "http://127.0.0.1:1/mcp");

    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = entry(&statuses, "status-dead").expect("entry present");
    assert_eq!(status.observed_state(), ServiceStatus::Stopped);
    assert!(status.tools_count.is_none());
}

// Why: the health verdict splits on a 1000ms connection budget, but both
// Healthy and Degraded report the service as running. A slow-but-alive server
// must not be reported stopped, or an operator restarts a service that is
// merely loaded.
#[tokio::test]
async fn a_slow_but_reachable_endpoint_is_still_reported_running_and_marked_degraded() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"jsonrpc": "2.0", "method": "initialize", "params": {"clientInfo": {}}}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-degraded")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "slow", "version": "1.0.0"}
                    }
                }))
                .set_delay(std::time::Duration::from_millis(1200)),
        )
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "params": {}}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": default_tools_json()}
                })),
        )
        .mount(&mock)
        .await;

    let config = external_mcp_config("status-slow", &format!("{}/mcp", mock.uri()));
    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = entry(&statuses, "status-slow").expect("entry present");
    assert_eq!(
        status.observed_state(),
        ServiceStatus::Running,
        "a reachable server over the latency budget must still read as running"
    );
    assert_eq!(
        status.health,
        HealthStatus::Degraded,
        "a reachable server over the latency budget must be flagged degraded"
    );
    assert!(
        status.latency_ms.is_some_and(|ms| ms >= 1000),
        "the reported latency must be the measured one: {:?}",
        status.latency_ms
    );
}

// Why: an operator reading the status table needs to know a service is
// auth-gated; the flag comes from config, not from the probe, so an
// unreachable server must still report it.
#[tokio::test]
async fn an_auth_gated_service_reports_auth_required_even_when_unreachable() {
    let mut config = external_mcp_config("status-authgated", "http://127.0.0.1:1/mcp");
    config.oauth.required = true;

    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = entry(&statuses, "status-authgated").expect("entry present");
    assert!(
        status.auth_required,
        "an oauth-gated service must report auth_required regardless of reachability"
    );
    assert_eq!(status.observed_state(), ServiceStatus::Stopped);
}

#[tokio::test]
async fn invalid_internal_endpoint_is_reported_unreachable_without_aborting_other_statuses() {
    let live = MockServer::start().await;
    mount_mcp_endpoint(&live, default_tools_json()).await;
    let healthy = external_mcp_config("status-neighbor", &format!("{}/mcp", live.uri()));
    let mut invalid = external_mcp_config("status-invalid-internal", "");
    invalid.server_type = systemprompt_models::mcp::McpServerType::Internal;
    invalid.port = None;
    invalid.oauth.required = true;

    let statuses = get_all_service_status(&[invalid, healthy])
        .await
        .expect("one malformed service is represented rather than aborting aggregation");
    let unreachable = entry(&statuses, "status-invalid-internal").expect("invalid service entry");
    assert_eq!(unreachable.observed_state(), ServiceStatus::Stopped);
    assert_eq!(unreachable.health, HealthStatus::Unhealthy);
    assert_eq!(unreachable.tools_count, None);
    assert_eq!(unreachable.latency_ms, None);
    assert!(unreachable.auth_required);
    let neighbor = entry(&statuses, "status-neighbor").expect("healthy neighbor");
    assert_eq!(neighbor.observed_state(), ServiceStatus::Running);
    assert_eq!(neighbor.health, HealthStatus::Healthy);
    assert_eq!(neighbor.tools_count, Some(2));
}
