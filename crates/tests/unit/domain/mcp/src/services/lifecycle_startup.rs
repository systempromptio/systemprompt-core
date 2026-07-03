//! Drives the startup wait/health-decision loop against a scripted MCP
//! endpoint: healthy readiness, degraded acceptance near exhaustion, dead-pid
//! detection, and the external-server spawn guard.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_mcp::services::LifecycleOrchestrator;
use systemprompt_mcp::services::lifecycle::startup::{check_health_status, wait_for_startup};
use systemprompt_models::mcp::McpServerConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, fixture_user_id,
};
use systemprompt_traits::startup_channel;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::harness::{
    default_tools_json, external_mcp_config, internal_mcp_config, mount_mcp_endpoint,
};

fn internal_at(mock: &MockServer, name: &str) -> McpServerConfig {
    let port = mock.address().port();
    internal_mcp_config(name, port)
}

#[tokio::test]
async fn wait_for_startup_reports_ready_on_healthy_endpoint() {
    let _ = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let config = internal_at(&mock, "startup-healthy");
    let (tx, _rx) = startup_channel();

    let startup_ms = wait_for_startup(&config, std::process::id(), Some(&tx))
        .await
        .expect("healthy endpoint ready");
    assert!(startup_ms.is_some());
}

#[tokio::test]
async fn wait_for_startup_detects_dead_process() {
    let _ = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let mut child = std::process::Command::new("true")
        .spawn()
        .expect("spawn short-lived child");
    let pid = child.id();
    child.wait().expect("child exits");

    let config = internal_at(&mock, "startup-dead");
    let err = wait_for_startup(&config, pid, None)
        .await
        .expect_err("dead pid detected");
    assert!(err.to_string().contains("died during startup"));
}

#[tokio::test]
async fn check_health_status_healthy_returns_elapsed() {
    let _ = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let config = internal_at(&mock, "health-decide");
    let (tx, _rx) = startup_channel();
    let start = std::time::Instant::now();

    let outcome = check_health_status(&config, 1, 15, start, Some(&tx))
        .await
        .expect("health check runs");
    assert!(outcome.is_some());
}

#[tokio::test]
async fn check_health_status_unreachable_is_not_ready() {
    let _ = ensure_test_bootstrap();
    let config = internal_mcp_config("health-unreachable", 1);
    let start = std::time::Instant::now();

    let outcome = check_health_status(&config, 1, 15, start, None)
        .await
        .expect("unreachable maps to not-ready");
    assert!(outcome.is_none());
}

#[tokio::test]
async fn check_health_status_accepts_degraded_near_exhaustion() {
    let _ = ensure_test_bootstrap();
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-slow")
                .set_delay(Duration::from_millis(1100))
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "slow", "version": "1.0.0"}
                    }
                })),
        )
        .mount(&mock)
        .await;
    mount_mcp_endpoint(&mock, default_tools_json()).await;

    let config = internal_at(&mock, "health-degraded");
    let start = std::time::Instant::now();

    let early = check_health_status(&config, 1, 15, start, None)
        .await
        .expect("slow endpoint early");
    assert!(early.is_none());

    let (tx, _rx) = startup_channel();
    let late = check_health_status(&config, 14, 15, start, Some(&tx))
        .await
        .expect("slow endpoint near exhaustion");
    assert!(late.is_some());
}

#[tokio::test]
async fn start_server_rejects_external_servers() {
    let _ = ensure_test_bootstrap();
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let bootstrap = ensure_test_bootstrap();
    let registry = systemprompt_mcp::services::registry::RegistryService::new(fixture_user_id());
    let database = systemprompt_mcp::services::database::DatabaseService::new(
        db,
        Arc::new(bootstrap.app_paths.clone()),
        registry,
    );
    let lifecycle = LifecycleOrchestrator::new(
        systemprompt_mcp::services::process::ProcessService::new(),
        systemprompt_mcp::services::NetworkService::new(),
        database,
        systemprompt_mcp::services::MonitoringService::new(),
        Arc::new(bootstrap.app_paths.clone()),
    );

    let config = external_mcp_config("startup-ext", "http://127.0.0.1:1/mcp");
    let err = lifecycle
        .start_server(&config)
        .await
        .expect_err("external servers are not spawned");
    assert!(err.to_string().contains("must not be spawned"));
}
