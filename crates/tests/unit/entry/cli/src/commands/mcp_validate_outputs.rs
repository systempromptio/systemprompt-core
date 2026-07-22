//! Tests for `plugins mcp validate` output builders and connection handling.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::plugins::mcp::validate::{
    FailureDetail, failure_output, prompt_server_selection, run_connection_validation,
    success_output,
};
use systemprompt_cli::ScriptedPrompter;
use systemprompt_mcp::services::client::{McpConnectionResult, McpProtocolInfo};
use systemprompt_models::{Deployment, ServicesConfig};

fn deployment(port: u16) -> Deployment {
    serde_yaml::from_str(&format!(
        "binary: test-server\npackage: null\nport: {port}\nenabled: true\ndisplay_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  client_id: null\n"
    ))
    .unwrap()
}

#[test]
fn failure_output_maps_detail_fields() {
    let out = failure_output(
        "svc",
        FailureDetail {
            health_status: "stopped",
            validation_type: "not_running",
            latency_ms: 42,
            issue: "Service is not currently running".to_owned(),
            message: "MCP server 'svc' is not running".to_owned(),
        },
    );

    assert_eq!(out.server, "svc");
    assert!(!out.valid);
    assert_eq!(out.health_status, "stopped");
    assert_eq!(out.validation_type, "not_running");
    assert_eq!(out.latency_ms, 42);
    assert_eq!(out.tools_count, 0);
    assert!(out.server_info.is_none());
    assert_eq!(out.issues, vec!["Service is not currently running"]);
    assert_eq!(out.message, "MCP server 'svc' is not running");
}

#[test]
fn success_output_copies_connection_result_and_server_info() {
    let result = McpConnectionResult {
        service_name: "svc".to_owned(),
        success: true,
        error_message: None,
        connection_time_ms: 12,
        server_info: Some(McpProtocolInfo {
            server_name: "demo".to_owned(),
            version: "1.2.3".to_owned(),
            protocol_version: "2025-06-18".to_owned(),
        }),
        tools_count: 4,
        validation_type: "full".to_owned(),
    };

    let out = success_output("svc", result);

    assert!(out.valid);
    assert_eq!(out.tools_count, 4);
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
        service_name: "svc".to_owned(),
        success: false,
        error_message: Some("handshake refused".to_owned()),
        connection_time_ms: 3,
        server_info: None,
        tools_count: 0,
        validation_type: "partial".to_owned(),
    };

    let out = success_output("svc", result);

    assert!(!out.valid);
    assert_eq!(out.issues, vec!["handshake refused"]);
    assert!(out.server_info.is_none());
}

#[test]
fn success_output_ignores_empty_error_message() {
    let result = McpConnectionResult {
        service_name: "svc".to_owned(),
        success: true,
        error_message: Some(String::new()),
        connection_time_ms: 1,
        server_info: None,
        tools_count: 0,
        validation_type: "partial".to_owned(),
    };

    assert!(success_output("svc", result).issues.is_empty());
}

#[tokio::test]
async fn run_connection_validation_reports_connection_error_for_closed_port() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let out = run_connection_validation("svc", &deployment(port), 30).await;

    assert!(!out.valid);
    assert_eq!(out.validation_type, "connection_failed");
    assert!(!out.issues.is_empty());
    assert_eq!(out.tools_count, 0);
}

#[tokio::test]
async fn run_connection_validation_times_out_against_silent_listener() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hold = tokio::spawn(async move {
        let mut sockets = Vec::new();
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            sockets.push(socket);
        }
    });

    let out = run_connection_validation("svc", &deployment(port), 1).await;
    hold.abort();

    assert!(!out.valid);
    assert_eq!(out.validation_type, "timeout");
    assert_eq!(out.latency_ms, 1000);
    assert!(out.issues[0].contains("timed out after 1 seconds"));
}

#[test]
fn prompt_server_selection_errors_without_servers_and_returns_sorted_choice() {
    let empty: ServicesConfig = serde_yaml::from_str("agents: {}\n").unwrap();
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let err = prompt_server_selection(&prompter, &empty).unwrap_err();
    assert!(err.to_string().contains("No MCP servers configured"));

    let config: ServicesConfig = serde_yaml::from_str(
        "mcp_servers:\n  zeta:\n    binary: b\n    package: null\n    port: 1\n    enabled: true\n    display_in_web: false\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: null\n  alpha:\n    binary: b\n    package: null\n    port: 2\n    enabled: true\n    display_in_web: false\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n      client_id: null\n",
    )
    .unwrap();
    let prompter = ScriptedPrompter::new(["1"]);
    let picked = prompt_server_selection(&prompter, &config).unwrap();
    assert_eq!(picked, "zeta");
}
