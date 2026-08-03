//! `MonitoringService` and `perform_health_check` over scripted endpoints.
//! Covers the external-server arms (accessor-backed short circuit and
//! endpoint-URL probe) and the three health verdicts the status roll-up maps
//! onto running / stopped / error.

use std::path::PathBuf;

use systemprompt_mcp::services::monitoring::MonitoringService;
use systemprompt_mcp::services::monitoring::health::{HealthStatus, perform_health_check};
use systemprompt_mcp::services::monitoring::status::display_service_status;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{ExternalAuth, McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::MockServer;

use crate::harness::{default_tools_json, mount_mcp_endpoint};

fn external_config(name: &str, endpoint: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::External,
        binary: String::new(),
        enabled: true,
        display_in_web: true,
        port: 0,
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: name.to_owned(),
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
        remote_endpoint: endpoint.to_owned(),
        external_auth: None,
        headers: Default::default(),
    }
}

async fn mcp_server_with(tools: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    mount_mcp_endpoint(&server, tools).await;
    server
}

#[tokio::test]
async fn health_check_of_an_accessor_backed_external_server_skips_the_probe() {
    let mock = mcp_server_with(default_tools_json()).await;
    let mut config = external_config("mon_accessor", &format!("{}/mcp", mock.uri()));
    config.external_auth = Some(ExternalAuth {
        token_endpoint: "/api/v1/tokens".to_owned(),
        header: "Authorization".to_owned(),
        scheme: "Bearer".to_owned(),
    });

    let result = perform_health_check(&config).await.expect("probe runs");

    assert_eq!(result.status, HealthStatus::Healthy);
    assert_eq!(result.details.validation_type, "external_accessor_backed");
    assert!(
        result.details.tools_available.is_none(),
        "an unprobed server has no enumerated tool count"
    );
    assert!(
        mock.received_requests().await.expect("recorded").is_empty(),
        "a per-user bearer cannot be minted here, so the endpoint is never called"
    );
}

#[tokio::test]
async fn health_check_of_an_external_server_probes_its_remote_endpoint() {
    let mock = mcp_server_with(default_tools_json()).await;
    let config = external_config("mon_ext_up", &format!("{}/mcp", mock.uri()));

    let result = perform_health_check(&config).await.expect("probe runs");

    assert_eq!(result.status, HealthStatus::Healthy);
    assert_eq!(result.details.tools_available, Some(2));
    assert!(
        mock.received_requests()
            .await
            .expect("recorded")
            .iter()
            .any(|r| String::from_utf8_lossy(&r.body).contains("initialize")),
        "the external probe performs the MCP handshake"
    );
}

#[tokio::test]
async fn health_check_of_an_unreachable_external_server_is_unhealthy() {
    let config = external_config("mon_ext_down", "http://127.0.0.1:9/mcp");

    let result = perform_health_check(&config).await.expect("probe runs");

    assert_eq!(result.status, HealthStatus::Unhealthy);
    assert!(
        result.details.error_message.is_some(),
        "the failure reason is carried through"
    );
}

#[tokio::test]
async fn health_check_of_a_server_reporting_no_tools_is_unknown() {
    let mock = mcp_server_with(serde_json::json!([])).await;
    let config = external_config("mon_ext_empty", &format!("{}/mcp", mock.uri()));

    let result = perform_health_check(&config).await.expect("probe runs");

    assert_eq!(
        result.status,
        HealthStatus::Unknown,
        "an empty tool list is indistinguishable from an auth-gated server"
    );
    assert_eq!(result.details.tools_available, Some(0));
}

#[tokio::test]
async fn check_health_returns_only_the_verdict() {
    let mock = mcp_server_with(default_tools_json()).await;
    let config = external_config("mon_verdict", &format!("{}/mcp", mock.uri()));

    let status = MonitoringService::new()
        .check_health(&config)
        .await
        .expect("check runs");

    assert_eq!(status, HealthStatus::Healthy);
}

#[tokio::test]
async fn status_for_all_maps_each_health_verdict_onto_a_service_state() {
    let up = mcp_server_with(default_tools_json()).await;
    let empty = mcp_server_with(serde_json::json!([])).await;

    let servers = vec![
        external_config("mon_all_up", &format!("{}/mcp", up.uri())),
        external_config("mon_all_empty", &format!("{}/mcp", empty.uri())),
        external_config("mon_all_down", "http://127.0.0.1:9/mcp"),
    ];

    let statuses = MonitoringService::new()
        .get_status_for_all(&servers)
        .await
        .expect("status roll-up runs");

    assert_eq!(statuses["mon_all_up"].state, "running");
    assert_eq!(statuses["mon_all_up"].tools_count, Some(2));
    assert!(!statuses["mon_all_up"].auth_required);

    assert_eq!(
        statuses["mon_all_empty"].state, "error",
        "an unknown verdict is an error state, not a stopped one"
    );
    assert_eq!(statuses["mon_all_down"].state, "stopped");

    display_service_status(&servers, &statuses);
    display_service_status(&[], &statuses);
}
