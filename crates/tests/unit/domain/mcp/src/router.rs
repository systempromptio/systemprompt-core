//! Tests for the crate-level MCP router assembly: default host allow-list,
//! request logging layer, and the buffering-suppression response header.

use axum::body::Body;
use systemprompt_mcp::{McpHttpConfig, SessionTimeouts, create_router};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use tower::ServiceExt;

#[derive(Debug, Clone)]
struct NullHandler;

impl rmcp::ServerHandler for NullHandler {}

#[test]
fn default_http_config_allows_local_hosts() {
    let config = McpHttpConfig::default();
    let hosts = config.allowed_hosts.expect("default allow-list");
    assert!(hosts.contains(&"localhost".to_owned()));
    assert!(hosts.contains(&"127.0.0.1".to_owned()));
    assert!(config.allowed_origins.is_empty());
    assert!(config.session.init.is_none());
    assert!(config.session.keep_alive.is_none());
}

#[tokio::test]
async fn router_serves_mcp_requests_with_logging_layers() {
    let _ = ensure_test_bootstrap();
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };

    let router = create_router(NullHandler, &db, McpHttpConfig::default());

    let response = router
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/mcp")
                .header("host", "127.0.0.1")
                .header("accept", "text/event-stream")
                .header("mcp-session-id", "sess-router")
                .header("x-proxy-verified", "true")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(
        response
            .headers()
            .get("x-accel-buffering")
            .and_then(|v| v.to_str().ok()),
        Some("no")
    );

    let denied = router
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "evil.example.com")
                .header("authorization", "Bearer t")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert!(!denied.status().is_success());
}

#[tokio::test]
async fn router_honours_disabled_host_allow_list() {
    let _ = ensure_test_bootstrap();
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };

    let config = McpHttpConfig {
        allowed_hosts: None,
        allowed_origins: vec!["http://ok.example".to_owned()],
        session: SessionTimeouts::default(),
    };
    let router = create_router(NullHandler, &db, config);

    let response = router
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/mcp")
                .header("host", "anything.example.com")
                .header("accept", "text/event-stream")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert!(response.status().as_u16() < 500);
}
