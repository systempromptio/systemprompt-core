//! Unit tests for the MCP service status model and its roll-up

use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::services::ServiceStatus;
use systemprompt_mcp::{HealthStatus, McpServiceStatus};
use systemprompt_models::mcp::McpServerType;

#[tokio::test]
async fn test_get_all_service_status_empty() {
    use systemprompt_mcp::services::monitoring::status::get_all_service_status;
    let statuses = get_all_service_status(&[]).await.unwrap();
    assert!(statuses.is_empty());
}

#[tokio::test]
async fn test_get_all_service_status_unreachable() {
    use std::path::PathBuf;
    use systemprompt_mcp::services::monitoring::status::get_all_service_status;
    use systemprompt_models::auth::JwtAudience;
    use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
    use systemprompt_models::mcp::server::McpServerConfig;
    use systemprompt_test_fixtures::fixture_user_id;

    let config = McpServerConfig {
        name: "unreach".to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: Some("x".to_owned()),
        enabled: true,
        display_in_web: true,
        port: Some(65529),
        crate_path: PathBuf::from("."),
        display_name: "x".to_owned(),
        description: "x".to_owned(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.0.1".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    };
    let statuses = get_all_service_status(&[config]).await.unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].name, "unreach");
    assert_eq!(statuses[0].observed_state(), ServiceStatus::Stopped);
}

#[test]
fn mcp_service_status_managed_row_carries_pid_and_port() {
    let status = McpServiceStatus {
        name: McpServerId::new("local"),
        server_type: McpServerType::Internal,
        port: Some(3010),
        endpoint: None,
        health: HealthStatus::Healthy,
        pid: Some(4242),
        tools_count: Some(7),
        latency_ms: Some(12),
        auth_required: false,
    };

    assert_eq!(status.server_type, McpServerType::Internal);
    assert_eq!(status.port, Some(3010));
    assert_eq!(status.pid, Some(4242));
    assert!(status.endpoint.is_none());
    assert_eq!(status.health, HealthStatus::Healthy);
    assert_eq!(status.observed_state(), ServiceStatus::Running);
}

#[test]
fn mcp_service_status_external_row_carries_endpoint_not_pid() {
    let status = McpServiceStatus {
        name: McpServerId::new("remote"),
        server_type: McpServerType::External,
        port: None,
        endpoint: Some("https://example.com/mcp".to_owned()),
        health: HealthStatus::Unhealthy,
        pid: None,
        tools_count: None,
        latency_ms: None,
        auth_required: true,
    };

    assert_eq!(status.server_type, McpServerType::External);
    assert_eq!(status.port, None);
    assert!(status.pid.is_none());
    assert_eq!(status.endpoint.as_deref(), Some("https://example.com/mcp"));
    assert_eq!(status.health, HealthStatus::Unhealthy);
}

#[test]
fn test_display_service_status_smoke() {
    use systemprompt_mcp::services::monitoring::status::display_service_status;

    display_service_status(&[]);
}
