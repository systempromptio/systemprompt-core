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

use systemprompt_bridge::ids::McpSessionId;
use systemprompt_bridge::proxy::mcp_probe::{
    McpAuthState, build_client, probe_all, probe_endpoint,
};
use systemprompt_bridge::proxy::{DEFAULT_PROXY_PORT, LoopbackEndpoint};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SLUG: &str = "myslug";
const BEARER: &str = "Bearer test";

// The exact JSON-RPC frames the probe sends, as the servers see them. A mount
// that matched on `method` alone let `"params": null` ship — Google's MCP
// refuses it — so every mount holds the whole request shape.
fn initialize_wire() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "systemprompt-bridge-probe" }
        }
    })
}

fn initialized_wire() -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

fn tools_list_wire() -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} })
}

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
        .and(body_partial_json(initialize_wire()))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "sess-123")
                .set_body_json(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": {} })),
        )
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialized_wire()))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(tools_list_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": { "tools": [{ "name": "a" }, { "name": "b" }] }
        })))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated);
    assert_eq!(
        auth.tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        auth.session_id.as_ref().map(McpSessionId::as_str),
        Some("sess-123")
    );
    assert_eq!(auth.http_status, Some(200));
    assert!(auth.error.is_none());
}

#[tokio::test]
async fn authenticated_sse() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialize_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialized_wire()))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    let sse = "event: message\n\
               data: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"sse_tool\"}]}}\n\n";
    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(tools_list_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated);
    assert_eq!(
        auth.tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["sse_tool"]
    );
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
async fn tools_list_failure_reports_protocol_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialize_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialized_wire()))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(tools_list_wire()))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::ProtocolError);
    assert!(auth.tools.is_empty());
}

// Google's Discovery Engine MCP answers `"params": null` with
// `-32602 Invalid params` at HTTP 200; the probe must send an object, and a
// JSON-RPC error must be named rather than reported as a missing tools array.
#[tokio::test]
async fn tools_list_params_is_an_object_not_null() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialize_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialized_wire()))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(tools_list_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": { "tools": [{ "name": "search" }] }
        })))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::Authenticated, "{:?}", auth.error);
    assert_eq!(auth.tools.len(), 1);

    let received = server.received_requests().await.expect("requests recorded");
    let tools_list = received
        .iter()
        .find_map(|r| {
            let body: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
            (body["method"] == "tools/list").then_some(body)
        })
        .expect("tools/list was sent");
    assert_eq!(tools_list["params"], serde_json::json!({}));
}

#[tokio::test]
async fn tools_list_json_rpc_error_is_named() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialize_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": {}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(initialized_wire()))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(body_partial_json(tools_list_wire()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": { "code": -32602, "message": "Invalid params" }
        })))
        .mount(&server)
        .await;

    let auth = probe(&server).await;
    assert_eq!(auth.state, McpAuthState::ProtocolError);
    let error = auth.error.expect("error is reported");
    assert!(
        error.contains("-32602") && error.contains("Invalid params"),
        "the JSON-RPC error is named: {error}"
    );
}

#[tokio::test]
async fn probe_all_empty_registry_yields_no_servers() {
    let results = probe_all(&loopback(), &std::collections::HashMap::new()).await;
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
    let gateway = "http://127.0.0.1:1";
    std::fs::write(
        meta.join("mcp-servers.json"),
        serde_json::to_vec(&serde_json::json!({ "gateway": gateway, "servers": servers }))
            .expect("servers json"),
    )
    .expect("write fragment");
    systemprompt_bridge::mcp_registry::rehydrate_from_disk(
        &REGISTRY,
        &systemprompt_identifiers::ValidatedUrl::try_new(gateway).expect("valid ValidatedUrl"),
    )
    .expect("rehydrate reads the seeded fragment");
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
            .block_on(probe_all(
                &loopback(),
                &systemprompt_bridge::mcp_registry::snapshot(&REGISTRY),
            ));

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
            // Not `ProxyUnreachable` exactly: the loopback port is process-wide,
            // and a sibling test in this shard runs a real proxy on it, which
            // turns the probe into a LoopbackMismatch. What this test owns is
            // the enumeration, not whether something happened to be listening.
            assert!(
                !matches!(entry.state, McpAuthState::Authenticated),
                "an unseeded probe cannot authenticate: {:?}",
                entry.state
            );
            assert!(entry.latency_ms.is_some(), "the attempt is timed");
            assert!(
                entry.tools.is_empty(),
                "no tool list is enumerated without authenticating: {:?}",
                entry.tools
            );
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

static REGISTRY: std::sync::LazyLock<
    std::sync::Arc<systemprompt_bridge::mcp_registry::McpRegistrySlot>,
> = std::sync::LazyLock::new(systemprompt_bridge::mcp_registry::empty_slot);

fn loopback() -> LoopbackEndpoint {
    LoopbackEndpoint::new(DEFAULT_PROXY_PORT, None)
}

#[tokio::test]
async fn coverage_probe_slug_only_contacts_registered_servers_and_forwards_the_secret() {
    use systemprompt_bridge::ids::LoopbackSecret;
    use systemprompt_bridge::proxy::mcp_probe::probe_slug;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let endpoint = LoopbackEndpoint::new(
        server.address().port(),
        Some(LoopbackSecret::new("sandbox-secret")),
    );
    let mut registry = systemprompt_bridge::mcp_registry::McpRegistry::new();
    assert!(probe_slug(&endpoint, &registry, "absent").await.is_none());
    registry.insert(
        "present".into(),
        systemprompt_bridge::mcp_registry::McpUpstream {
            url: server.uri().parse().unwrap(),
            headers: Default::default(),
            display_name: "Present".into(),
            transport: None,
            tool_policy: Default::default(),
        },
    );
    let result = probe_slug(&endpoint, &registry, "present").await.unwrap();
    assert_eq!(result.id, "present");
    assert_eq!(result.state, McpAuthState::GatewayUnauthorized);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].headers["authorization"],
        "Bearer sandbox-secret"
    );
    assert_eq!(requests[0].url.path(), "/mcp/present");
}

#[tokio::test]
async fn coverage_probe_timeout_is_distinct_from_an_authentication_rejection() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(1)))
        .mount(&server)
        .await;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(20))
        .build()
        .unwrap();
    let result = probe_endpoint(&client, SLUG, &target(&server), BEARER).await;
    assert_eq!(result.state, McpAuthState::ProbeTimeout);
    assert!(!result.state.is_conclusive());
    assert!(result.http_status.is_none());
}

#[tokio::test]
async fn coverage_probe_invalid_url_is_a_protocol_error() {
    let result = probe_endpoint(&build_client().unwrap(), SLUG, "not a URL", BEARER).await;
    assert_eq!(result.state, McpAuthState::ProtocolError);
    assert!(result.error.is_some());
}

#[tokio::test]
async fn coverage_probe_error_snippet_preserves_multibyte_characters() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(502).set_body_string(format!(" {} ", "€".repeat(100))))
        .mount(&server)
        .await;
    let result = probe(&server).await;
    assert_eq!(result.error.unwrap(), format!("{}…", "€".repeat(66)));
}

#[test]
fn coverage_probe_unwritable_secret_is_a_local_error_without_a_network_attempt() {
    let state = tempfile::tempdir().unwrap();
    state_sandbox(&state, || {
        std::fs::write(state.path().join("systemprompt"), "blocks config directory").unwrap();
        seed_registry(state.path(), &["one"]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let registry = systemprompt_bridge::mcp_registry::snapshot(&REGISTRY);
        let results = runtime.block_on(probe_all(&loopback(), &registry));
        assert_eq!(results[0].state, McpAuthState::LocalError);
        assert!(
            results[0]
                .error
                .as_deref()
                .unwrap()
                .contains("loopback secret unavailable")
        );
        assert!(results[0].latency_ms.is_none());
    });
}

async fn mount_successful_probe(server: &MockServer, session: &str, tool: &str) {
    Mock::given(method("POST"))
        .and(path(format!("/mcp/{SLUG}")))
        .and(wiremock::matchers::header("authorization", BEARER))
        .and(body_partial_json(initialize_wire()))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", session)
                .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
        )
        .expect(1)
        .mount(server)
        .await;
    for body in [initialized_wire(), tools_list_wire()] {
        let response = if body["method"] == "tools/list" {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc":"2.0","id":2,"result":{"tools":[{"name":tool}]}
            }))
        } else {
            ResponseTemplate::new(202)
        };
        Mock::given(method("POST"))
            .and(path(format!("/mcp/{SLUG}")))
            .and(wiremock::matchers::header("authorization", BEARER))
            .and(wiremock::matchers::header("mcp-session-id", session))
            .and(body_partial_json(body))
            .respond_with(response)
            .expect(1)
            .mount(server)
            .await;
    }
}

async fn assert_successful_probe_wire(server: &MockServer, session: &str) {
    let requests = server.received_requests().await.expect("probe requests");
    assert_eq!(requests.len(), 3, "initialize, initialized, tools/list");
    assert!(requests.iter().all(|request| {
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            == Some(BEARER)
    }));
    for request in requests {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let carried = request
            .headers
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok());
        if body["method"] == "initialize" {
            assert_eq!(
                carried, None,
                "initialize cannot claim a session before it exists"
            );
        } else {
            assert_eq!(
                carried,
                Some(session),
                "followup frame is scoped to initialized session"
            );
        }
    }
}

#[tokio::test]
async fn initialized_notification_failure_stops_before_tools_then_a_retry_recovers() {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(initialize_wire()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("mcp-session-id", "retry-session")
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(initialized_wire()))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(tools_list_wire()))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let failed = probe(&server).await;
        assert_eq!(failed.state, McpAuthState::ProtocolError);
        assert!(failed.error.as_deref().is_some_and(|e| e.contains("503")));
        assert!(failed.tools.is_empty());

        server.reset().await;
        mount_successful_probe(&server, "retry-session", "recovered-tool").await;
        let recovered = probe(&server).await;
        assert_eq!(recovered.state, McpAuthState::Authenticated);
        assert_eq!(recovered.tools[0].name, "recovered-tool");
        assert_eq!(
            recovered.session_id.as_ref().map(McpSessionId::as_str),
            Some("retry-session")
        );
        assert_successful_probe_wire(&server, "retry-session").await;
    })
    .await
    .expect("probe recovery completes within ten seconds");
}

#[tokio::test]
async fn malformed_tools_response_does_not_reuse_partial_tools_and_retry_recovers() {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(initialize_wire()))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":{}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(initialized_wire()))
            .respond_with(ResponseTemplate::new(202))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(body_partial_json(tools_list_wire()))
            .respond_with(ResponseTemplate::new(200).set_body_string("{truncated"))
            .expect(1)
            .mount(&server)
            .await;
        let failed = probe(&server).await;
        assert_eq!(failed.state, McpAuthState::ProtocolError);
        assert!(
            failed
                .error
                .as_deref()
                .is_some_and(|e| e.contains("response"))
        );
        assert!(failed.tools.is_empty());

        server.reset().await;
        mount_successful_probe(&server, "clean-session", "clean-tool").await;
        let recovered = probe(&server).await;
        assert_eq!(
            recovered
                .tools
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["clean-tool"]
        );
        assert_successful_probe_wire(&server, "clean-session").await;
    })
    .await
    .expect("malformed response recovery completes within ten seconds");
}
