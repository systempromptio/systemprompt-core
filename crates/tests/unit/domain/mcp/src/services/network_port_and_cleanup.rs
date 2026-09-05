//! Port-range search and process-cleanup verbs, plus the MCP validation verdict
//! for a server that completes the handshake but fails `tools/list`. Children
//! carry this crate's spawn markers so the identity-gated terminate path
//! signals a real process rather than relying on the test process's own pid.

use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_mcp::services::client::validate_connection_by_url;
use systemprompt_mcp::services::network::port::{find_available_port, is_port_in_use};
use systemprompt_mcp::services::process::cleanup::{
    cleanup_port_processes, force_kill, terminate_gracefully, terminate_gracefully_verified,
};
use systemprompt_mcp::services::process::monitor::is_process_running;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

// The process is orphaned rather than kept as a direct child: a killed child
// this process has not `wait`ed on lingers as a zombie and still answers
// `kill(pid, 0)`. Its stdout is redirected away from the captured pipe, which
// an orphan would otherwise hold open for its whole lifetime.
fn spawn_marked_orphan(service_name: &str) -> Option<u32> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("sleep 600 >/dev/null 2>&1 & echo $!")
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
            service_name,
        )
        .output()
        .ok()?;

    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if is_process_running(pid) {
            return Some(pid);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn died_within(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !is_process_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn unique_service(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

#[test]
fn find_available_port_skips_a_port_this_test_is_holding() {
    let (listener, port) = held_port();

    let found = find_available_port(port, port.saturating_add(20)).expect("a free port exists");
    drop(listener);

    assert_ne!(found, port, "the held port is not offered as available");
    assert!(found > port, "the search walks forward from the start port");
}

#[test]
fn find_available_port_reports_a_range_with_nothing_free() {
    let (listener, port) = held_port();

    let result = find_available_port(port, port);
    drop(listener);

    let err = result.expect_err("a single-port range that is occupied has no answer");
    assert!(
        err.to_string().contains(&format!("{port}-{port}")),
        "the failure names the exhausted range: {err}"
    );
}

#[test]
fn is_port_in_use_tracks_a_listener_opening_and_closing() {
    let (listener, port) = held_port();
    assert!(is_port_in_use(port), "a bound port probes as in use");

    drop(listener);
    assert!(!is_port_in_use(port), "a closed port probes as free");
}

#[test]
fn terminate_gracefully_is_a_no_op_for_a_process_that_is_already_gone() {
    terminate_gracefully(u32::MAX).expect("an absent process needs no signal");
    force_kill(u32::MAX).expect("an absent process needs no kill");
}

#[tokio::test]
async fn terminate_gracefully_verified_signals_a_child_carrying_the_service_marker() {
    let service = unique_service("mcpsig");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_orphan(&service) else {
        return;
    };

    let result = terminate_gracefully_verified(pid, &service).await;
    let _ = force_kill(pid);

    result.expect("our own process terminates");
    assert!(
        died_within(pid, Duration::from_secs(2)),
        "the marked process was signalled"
    );
}

#[tokio::test]
async fn terminate_gracefully_verified_leaves_a_pid_naming_another_service_alone() {
    let service = unique_service("mcpmine");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_orphan(&service) else {
        return;
    };

    let result = terminate_gracefully_verified(pid, &unique_service("mcpother")).await;
    let still_alive = is_process_running(pid);
    let _ = force_kill(pid);

    result.expect("a stale pid is reported as handled, not an error");
    assert!(
        still_alive,
        "a pid whose environ names another service must never be signalled"
    );
}

#[tokio::test]
async fn cleanup_port_processes_reports_nothing_for_a_port_with_no_holder() {
    let (listener, port) = held_port();
    drop(listener);

    assert!(
        cleanup_port_processes(port)
            .await
            .expect("the sweep runs")
            .is_empty(),
        "an unheld port yields no pids to clean"
    );
}

#[tokio::test]
async fn validation_reports_tools_request_failed_when_the_server_rejects_tools_list() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
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
        .and(body_partial_json(serde_json::json!({
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "tools/list"}),
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

    let result = validate_connection_by_url("refuser", &format!("{}/mcp", server.uri()))
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
