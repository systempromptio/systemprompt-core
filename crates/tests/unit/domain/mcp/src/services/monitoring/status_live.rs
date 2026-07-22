use systemprompt_mcp::services::monitoring::status::{
    display_service_status, get_all_service_status,
};
use wiremock::MockServer;

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
    assert_eq!(status.tools_count, 2);
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
    assert_eq!(status.tools_count, 0);
}
