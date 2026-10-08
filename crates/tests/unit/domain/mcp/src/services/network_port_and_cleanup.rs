//! Port probing and reclamation, plus the MCP validation verdict for a server
//! that completes the handshake but fails `tools/list`. The identity-gated
//! stop of a marked child is covered in `process::cleanup_live`.

use std::net::TcpListener;
use systemprompt_identifiers::ServiceName;

use systemprompt_mcp::services::client::validate_connection_by_url;
use systemprompt_mcp::services::network::port::{cleanup_port_processes, is_port_in_use};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn is_port_in_use_tracks_a_listener_opening_and_closing() {
    let (listener, port) = held_port();
    assert!(is_port_in_use(port).await, "a bound port probes as in use");

    drop(listener);
    assert!(!is_port_in_use(port).await, "a closed port probes as free");
}

#[tokio::test]
async fn cleanup_port_processes_is_a_no_op_for_a_port_with_no_holder() {
    let (listener, port) = held_port();
    drop(listener);

    cleanup_port_processes(port, &ServiceName::new("unheld"))
        .await
        .expect("an unheld port yields nothing to clean");
}

#[tokio::test]
async fn cleanup_port_processes_never_looks_up_port_zero() {
    cleanup_port_processes(0, &ServiceName::new("port-zero"))
        .await
        .expect("port 0 has no holder to clean");
}

#[tokio::test]
async fn validation_reports_tools_request_failed_when_the_server_rejects_tools_list() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"jsonrpc": "2.0", "method": "initialize", "params": {"clientInfo": {}}}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-fail")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "refuser", "version": "9.9.9"}
                    }
                })),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
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
                    "error": {"code": -32000, "message": "tools are unavailable"}
                })),
        )
        .mount(&server)
        .await;

    let result = validate_connection_by_url(
        &ServiceName::new("refuser"),
        &format!("{}/mcp", server.uri()),
    )
    .await
    .expect("the probe completes");

    assert!(!result.success);
    assert_eq!(
        result.validation_type, "tools_request_failed",
        "a handshake that succeeds but cannot list tools is distinct from an unreachable port"
    );
    assert!(
        result.tools_count.is_none(),
        "no tool count is claimed when the request failed"
    );
    assert_eq!(
        result.server_info.map(|info| info.version),
        Some("9.9.9".to_owned()),
        "the peer info gathered before the failure is still reported"
    );
}
