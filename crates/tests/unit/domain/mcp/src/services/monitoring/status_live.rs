use systemprompt_mcp::services::monitoring::status::{
    display_service_status, get_all_service_status,
};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::harness::{default_tools_json, external_mcp_config, mount_mcp_endpoint};

#[tokio::test]
async fn live_endpoint_reports_running_with_tool_count() {
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;
    let config = external_mcp_config("status-live", &format!("{}/mcp", mock.uri()));

    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = statuses.get("status-live").expect("entry present");
    assert_eq!(status.state, "running");
    assert_eq!(status.health, "healthy");
    assert_eq!(status.tools_count, Some(2));
    assert!(status.latency_ms.is_some());
    assert!(!status.auth_required);

    display_service_status(std::slice::from_ref(&config), &statuses);
}

#[tokio::test]
async fn dead_endpoint_reports_stopped() {
    let config = external_mcp_config("status-dead", "http://127.0.0.1:1/mcp");

    let statuses = get_all_service_status(std::slice::from_ref(&config))
        .await
        .expect("status map builds");

    let status = statuses.get("status-dead").expect("entry present");
    assert_eq!(status.state, "stopped");
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
            serde_json::json!({"method": "initialize"}),
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
        .and(body_partial_json(serde_json::json!({
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({"method": "tools/list"})))
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

    let status = statuses.get("status-slow").expect("entry present");
    assert_eq!(
        status.state, "running",
        "a reachable server over the latency budget must still read as running"
    );
    assert_eq!(
        status.health, "degraded",
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

    let status = statuses.get("status-authgated").expect("entry present");
    assert!(
        status.auth_required,
        "an oauth-gated service must report auth_required regardless of reachability"
    );
    assert_eq!(status.state, "stopped");
}
