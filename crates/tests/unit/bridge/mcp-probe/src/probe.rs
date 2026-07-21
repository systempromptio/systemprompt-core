//! Integration tests for the live MCP auth probe against a `wiremock` server.
//!
//! Each test stands up a `MockServer` standing in for "loopback proxy +
//! upstream MCP server", programs the JSON-RPC calls the probe issues on the
//! single `/mcp/<slug>` POST path, and drives [`probe_endpoint`] with the
//! production client builder, injecting the URL and bearer directly.
//!
//! The `probe_all` registry walk is the exception: it reads the process-global
//! registry and loopback secret, so it lives in a single sequential test that
//! owns both.

use systemprompt_bridge::proxy::mcp_probe::{
    McpAuthState, build_client, probe_all, probe_endpoint,
};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SLUG: &str = "myslug";
const BEARER: &str = "Bearer test";

fn target(server: &MockServer) -> String {
    format!("{}/mcp/{SLUG}", server.uri())
}

async fn probe(server: &MockServer) -> systemprompt_bridge::proxy::mcp_probe::McpServerAuth {
    let client = build_client().expect("probe client builds");
    probe_endpoint(&client, SLUG, &target(server), BEARER).await
}

#[tokio::test]
async fn authenticated_json() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "initialize" }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "sess-123")
                .set_body_json(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} })),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "notifications/initialized" }),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "tools/list" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": { "tools": [{ "name": "a" }, { "name": "b" }] }
        })))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated);
    assert_eq!(auth.tools, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(auth.session_id.as_deref(), Some("sess-123"));
    assert_eq!(auth.http_status, Some(200));
    assert!(auth.error.is_none());
}

#[tokio::test]
async fn authenticated_sse() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "initialize" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "notifications/initialized" }),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    let sse = "event: message\n\
               data: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"sse_tool\"}]}}\n\n";
    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "tools/list" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated);
    assert_eq!(auth.tools, vec!["sse_tool".to_owned()]);
}

async fn assert_error_status(status: u16, expected: McpAuthState) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .respond_with(ResponseTemplate::new(status).set_body_string("denied: nope"))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, expected, "status {status}");
    assert_eq!(auth.http_status, Some(status));
    assert!(auth.error.as_deref().is_some_and(|e| e.contains("denied")));
    assert!(auth.tools.is_empty());
}

#[tokio::test]
async fn status_403_loopback_mismatch() {
    assert_error_status(403, McpAuthState::LoopbackMismatch).await;
}

#[tokio::test]
async fn status_401_gateway_unauthorized() {
    assert_error_status(401, McpAuthState::GatewayUnauthorized).await;
}

#[tokio::test]
async fn status_404_not_registered() {
    assert_error_status(404, McpAuthState::NotRegistered).await;
}

#[tokio::test]
async fn status_500_upstream_error() {
    assert_error_status(500, McpAuthState::UpstreamError).await;
}

#[tokio::test]
async fn snippet_truncates_long_error_body() {
    let server = MockServer::start().await;
    let long = "x".repeat(500);
    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .respond_with(ResponseTemplate::new(500).set_body_string(long))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    let error = auth.error.expect("error populated");
    assert!(
        error.ends_with('…'),
        "expected truncation ellipsis, got: {error}"
    );
}

#[tokio::test]
async fn proxy_unreachable_on_closed_port() {
    let client = build_client().expect("probe client builds");
    let auth = probe_endpoint(&client, SLUG, "http://127.0.0.1:1/mcp/x", BEARER).await;
    assert_eq!(auth.state, McpAuthState::ProxyUnreachable);
    assert!(auth.error.is_some());
}

#[tokio::test]
async fn tools_list_failure_does_not_downgrade() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "initialize" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "notifications/initialized" }),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(
            serde_json::json!({ "method": "tools/list" }),
        ))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated);
    assert!(auth.tools.is_empty());
}

#[tokio::test]
async fn probe_all_empty_registry_yields_no_servers() {
    let results = probe_all().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].state, McpAuthState::NoServers);
}


fn state_sandbox<R>(state: &tempfile::TempDir, f: impl FnOnce() -> R) -> R {
    let root = state.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("XDG_STATE_HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(root.clone())),
            ("XDG_CACHE_HOME", Some(root.clone())),
            ("HOME", Some(root)),
        ],
        f,
    )
}

fn seed_registry(state: &std::path::Path, names: &[&str]) {
    let meta = state.join("systemprompt-bridge").join("metadata");
    std::fs::create_dir_all(&meta).expect("metadata dir");
    let servers: Vec<serde_json::Value> = names
        .iter()
        .map(|n| {
            serde_json::json!({
                "name": n,
                "url": "http://127.0.0.1:9/mcp",
                "transport": "http",
            })
        })
        .collect();
    std::fs::write(
        meta.join("mcp-servers.json"),
        serde_json::to_vec(&servers).expect("servers json"),
    )
    .expect("write fragment");
    systemprompt_bridge::mcp_registry::rehydrate_from_disk();
}

#[test]
fn probe_all_walks_the_registry_and_reports_each_server() {
    let state = tempfile::tempdir().expect("state dir");
    state_sandbox(&state, || {
        seed_registry(state.path(), &["Zulu MCP", "Alpha MCP"]);
        let results = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(probe_all());

        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["alpha-mcp", "zulu-mcp"],
            "every registered slug is probed, in sorted order"
        );
        for entry in &results {
            assert!(
                entry.url.ends_with(&format!("/mcp/{}", entry.id)),
                "each result carries its own loopback MCP URL: {}",
                entry.url
            );
            assert!(
                matches!(entry.state, McpAuthState::ProxyUnreachable),
                "no proxy is listening, so every server reads as unreachable: {:?}",
                entry.state
            );
            assert!(entry.latency_ms.is_some(), "the attempt is timed");
            assert!(entry.http_status.is_none());
            assert!(entry.tools.is_empty());
        }

        assert!(
            state
                .path()
                .join("systemprompt")
                .join("bridge-loopback.key")
                .is_file(),
            "the probe mints the loopback secret it needs"
        );
    });
}
