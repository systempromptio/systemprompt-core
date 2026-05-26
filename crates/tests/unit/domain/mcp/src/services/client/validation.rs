//! Unit tests for the MCP client validation helpers.
//!
//! These exercise the timeout / connection-failure / port-unreachable branches
//! without needing a real MCP server. Successful-path coverage is left to the
//! orchestrator integration tests.

use systemprompt_mcp::services::client::{
    validate_connection, validate_connection_by_url, validate_connection_with_auth,
};

const UNREACHABLE_HOST: &str = "127.0.0.1";

fn unused_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[tokio::test]
async fn validate_connection_by_url_invalid_uri_returns_failure() {
    let result = validate_connection_by_url("svc", "not://a/valid uri").await;
    let r = result.expect("returns Ok with failure result");
    assert!(!r.success);
    assert_eq!(r.service_name, "svc");
    assert!(r.error_message.is_some());
    assert!(matches!(
        r.validation_type.as_str(),
        "connection_failed" | "timeout"
    ));
    assert_eq!(r.tools_count, 0);
    assert!(r.server_info.is_none());
}

#[tokio::test]
async fn validate_connection_unreachable_port_returns_failure() {
    let port = unused_port();
    let r = validate_connection("svc-unreach", UNREACHABLE_HOST, port)
        .await
        .expect("returns Ok with failure result");
    assert!(!r.success);
    assert!(r.error_message.is_some());
}

#[tokio::test]
async fn validate_connection_with_auth_requires_oauth_port_open() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let r = validate_connection_with_auth("svc-oauth-up", UNREACHABLE_HOST, port, true)
        .await
        .expect("returns Ok with auth-required result");
    assert!(r.success);
    assert_eq!(r.validation_type, "auth_required");
    assert!(r.server_info.is_some());
    drop(listener);
}

#[tokio::test]
async fn validate_connection_with_auth_requires_oauth_port_closed() {
    let port = unused_port();
    let r = validate_connection_with_auth("svc-oauth-down", UNREACHABLE_HOST, port, true)
        .await
        .expect("returns Ok with port-unavailable");
    assert!(!r.success);
    assert_eq!(r.validation_type, "port_unavailable");
}

#[tokio::test]
async fn validate_connection_with_auth_no_oauth_routes_to_validate_connection() {
    let port = unused_port();
    let r = validate_connection_with_auth("svc-noauth", UNREACHABLE_HOST, port, false)
        .await
        .expect("returns Ok");
    assert!(!r.success);
}
