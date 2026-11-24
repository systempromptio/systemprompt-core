//! Tests for `plugins mcp validate` output builders and connection handling.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::ScriptedPrompter;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::ServicesConfig;
use systemprompt_mcp::services::client::{McpConnectionResult, McpHealthStatus, McpProtocolInfo};
use systemprompt_mcp_entry::cli::validate::{
    FailureDetail, failure_output, run_connection_validation, success_output,
};

#[test]
fn failure_output_maps_detail_fields() {
    let out = failure_output(
        &ServiceName::new("svc"),
        FailureDetail {
            health_status: McpHealthStatus::Stopped,
            validation_type: "not_running",
            latency_ms: 42,
            issue: "Service is not currently running".to_owned(),
            message: "MCP server 'svc' is not running".to_owned(),
        },
    );

    assert_eq!(out.server, "svc");
    assert!(!out.valid);
    assert_eq!(out.health_status, McpHealthStatus::Stopped);
    assert_eq!(out.validation_type, "not_running");
    assert_eq!(out.latency_ms, 42);
    assert!(out.tools_count.is_none());
    assert!(out.server_info.is_none());
    assert_eq!(out.issues, vec!["Service is not currently running"]);
    assert_eq!(out.message, "MCP server 'svc' is not running");
}

#[test]
fn success_output_copies_connection_result_and_server_info() {
    let result = McpConnectionResult {
        service_name: ServiceName::new("svc"),
        success: true,
        error_message: None,
        connection_time_ms: 12,
        server_info: Some(McpProtocolInfo {
            implementation_name: "demo".to_owned(),
            version: "1.2.3".to_owned(),
            protocol_version: "2025-06-18".to_owned(),
        }),
        tools_count: Some(4),
        validation_type: "full".to_owned(),
    };

    let out = success_output(&ServiceName::new("svc"), result);

    assert!(out.valid);
    assert_eq!(out.tools_count, Some(4));
    assert_eq!(out.latency_ms, 12);
    assert_eq!(out.validation_type, "full");
    let info = out.server_info.unwrap();
    assert_eq!(info.name, "demo");
    assert_eq!(info.version, "1.2.3");
    assert_eq!(info.protocol_version, "2025-06-18");
    assert!(out.issues.is_empty());
}

#[test]
fn success_output_surfaces_error_message_as_issue() {
    let result = McpConnectionResult {
        service_name: ServiceName::new("svc"),
        success: false,
        error_message: Some("handshake refused".to_owned()),
        connection_time_ms: 3,
        server_info: None,
        tools_count: None,
        validation_type: "partial".to_owned(),
    };

    let out = success_output(&ServiceName::new("svc"), result);

    assert!(!out.valid);
    assert_eq!(out.issues, vec!["handshake refused"]);
    assert!(out.server_info.is_none());
}

#[test]
fn success_output_ignores_empty_error_message() {
    let result = McpConnectionResult {
        service_name: ServiceName::new("svc"),
        success: true,
        error_message: Some(String::new()),
        connection_time_ms: 1,
        server_info: None,
        tools_count: None,
        validation_type: "partial".to_owned(),
    };

    assert!(
        success_output(&ServiceName::new("svc"), result)
            .issues
            .is_empty()
    );
}

#[tokio::test]
async fn run_connection_validation_accepts_a_listening_hosted_service() {
    let reserved = systemprompt_test_fixtures::ReservedPort::loopback();
    let port = reserved.port();
    let listener = reserved.into_tokio_listener();
    let accepted = tokio::spawn(async move {
        let (_socket, peer) = listener.accept().await.unwrap();
        peer
    });

    let out = run_connection_validation(&ServiceName::new("oauth-svc"), port, 1).await;
    let peer = accepted.await.unwrap();

    assert!(peer.ip().is_loopback());
    assert!(out.valid);
    assert_eq!(out.health_status, McpHealthStatus::AuthRequired);
    assert_eq!(out.validation_type, "auth_required");
    assert_eq!(out.server, "oauth-svc");
    assert_eq!(out.server_info.unwrap().name, "oauth-svc");
}

#[tokio::test]
async fn run_connection_validation_reports_an_unavailable_port() {
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();

    let out = run_connection_validation(&ServiceName::new("oauth-svc"), port, 1).await;

    assert!(!out.valid);
    assert_eq!(out.health_status, McpHealthStatus::Unhealthy);
    assert_eq!(out.validation_type, "port_unavailable");
    assert!(out.issues[0].contains("Port not responding"));
}

#[test]
fn prompt_server_selection_errors_without_servers_and_returns_sorted_choice() {
    let empty: ServicesConfig = serde_yaml::from_str("agents: {}\n").unwrap();
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = systemprompt_mcp_entry::cli::prompt_server_selection(
        &prompter,
        &empty,
        "Select MCP server to validate",
        ServiceName::new,
    )
    .unwrap_err();
    assert!(err.to_string().contains("No MCP servers configured"));

    let config: ServicesConfig = serde_yaml::from_str(
        "mcp_servers:\n  zeta:\n    binary: b\n    package: null\n    port: 1\n    enabled: true\n    display_in_web: false\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: null\n  alpha:\n    binary: b\n    package: null\n    port: 2\n    enabled: true\n    display_in_web: false\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: null\n",
    )
    .unwrap();
    let prompter = ScriptedPrompter::new(["1"]);
    let picked = systemprompt_mcp_entry::cli::prompt_server_selection(
        &prompter,
        &config,
        "Select MCP server to validate",
        ServiceName::new,
    )
    .unwrap();
    assert_eq!(picked, "zeta");
}
