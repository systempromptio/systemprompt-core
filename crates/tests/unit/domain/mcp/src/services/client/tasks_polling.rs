//! Drives the `io.modelcontextprotocol/tasks` client-side polling loop through
//! `McpClient::call_tool`.
//!
//! The endpoint is scripted rather than templated because the poll loop issues
//! a fresh `tasks/get` per round: rmcp routes each response by its JSON-RPC id,
//! so a canned body with a fixed id is delivered to nobody and the call hangs.
//! The responder echoes the request's own id and walks a queue of payloads,
//! repeating the last one once the queue is down to its final entry.

use std::sync::Mutex;

use serde_json::{Value, json};
use systemprompt_mcp::services::client::McpClient;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use crate::harness::{external_mcp_config, request_context};

struct ScriptedTaskServer {
    task_handle: Value,
    get_task_queue: Mutex<Vec<Value>>,
}

impl ScriptedTaskServer {
    fn new(ttl_ms: Option<u64>, get_task_payloads: Vec<Value>) -> Self {
        Self {
            task_handle: task_handle(ttl_ms),
            get_task_queue: Mutex::new(get_task_payloads),
        }
    }

    fn next_get_task(&self) -> Value {
        let mut queue = self.get_task_queue.lock().expect("queue lock");
        if queue.len() > 1 {
            queue.remove(0)
        } else {
            queue.first().cloned().unwrap_or_else(|| json!({}))
        }
    }
}

impl Respond for ScriptedTaskServer {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let rpc_method = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if rpc_method.starts_with("notifications/") {
            return ResponseTemplate::new(202);
        }
        let id = body.get("id").cloned().unwrap_or_else(|| json!(0));

        let payload = match rpc_method {
            "initialize" => json!({
                "protocolVersion": "2026-07-28",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "scripted-tasks", "version": "1.0.0"}
            }),
            "tools/call" => self.task_handle.clone(),
            "tasks/get" => self.next_get_task(),
            _ => json!({}),
        };

        // Why: a *task* payload legitimately carries its own `error` object
        // (a failed task), so the transport-level error is marked separately.
        let envelope = payload.get(RPC_ERROR_MARKER).map_or_else(
            || json!({"jsonrpc": "2.0", "id": id, "result": payload}),
            |error| json!({"jsonrpc": "2.0", "id": id, "error": error}),
        );

        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .insert_header("mcp-session-id", "sess-tasks")
            .set_body_json(envelope)
    }
}

const RPC_ERROR_MARKER: &str = "__rpc_error";

fn task_handle(ttl_ms: Option<u64>) -> Value {
    json!({
        "resultType": "task",
        "taskId": "task-1",
        "status": "working",
        "createdAt": "2026-01-01T00:00:00Z",
        "lastUpdatedAt": "2026-01-01T00:00:00Z",
        "ttlMs": ttl_ms,
        "pollIntervalMs": 10
    })
}

fn get_task(status_payload: Value) -> Value {
    let mut body = json!({
        "resultType": "complete",
        "taskId": "task-1",
        "createdAt": "2026-01-01T00:00:00Z",
        "lastUpdatedAt": "2026-01-01T00:00:00Z",
        "ttlMs": null,
        "pollIntervalMs": 10
    });
    let obj = body.as_object_mut().expect("object body");
    for (key, value) in status_payload.as_object().expect("payload object") {
        obj.insert(key.clone(), value.clone());
    }
    body
}

fn completed(result: Value) -> Value {
    get_task(json!({"status": "completed", "result": result}))
}

fn working() -> Value {
    get_task(json!({"status": "working"}))
}

fn text_result(text: &str) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

async fn call_echo(
    tag: &str,
    ttl_ms: Option<u64>,
    get_task_payloads: Vec<Value>,
) -> systemprompt_mcp::McpDomainResult<systemprompt_models::CallToolResult> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ScriptedTaskServer::new(ttl_ms, get_task_payloads))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = external_mcp_config(tag, &format!("{}/mcp", server.uri()));
    let outcome =
        McpClient::call_tool(&config, "echo".to_owned(), None, &request_context(tag)).await;
    drop(server);
    outcome
}

#[tokio::test]
async fn a_task_that_reports_working_before_completing_is_polled_until_its_result_arrives() {
    let _ = ensure_test_bootstrap();

    let result = call_echo(
        "tasks_ok",
        None,
        vec![working(), completed(text_result("task output"))],
    )
    .await
    .expect("task completes");

    assert_eq!(result.is_error, Some(false));
    let text = serde_json::to_string(&result.content).expect("content serializes");
    assert!(
        text.contains("task output"),
        "the polled task's own result must be returned, got {text}"
    );
}

// Why: `input_required` is a mid-flight state, not a terminal one. Treating it
// as terminal would abort a task that is merely waiting on a human.
#[tokio::test]
async fn a_task_awaiting_input_keeps_polling_rather_than_erroring() {
    let _ = ensure_test_bootstrap();

    let result = call_echo(
        "tasks_input",
        None,
        vec![
            get_task(json!({"status": "input_required", "inputRequests": {}})),
            completed(text_result("after input")),
        ],
    )
    .await
    .expect("task completes after input_required");

    let text = serde_json::to_string(&result.content).expect("content serializes");
    assert!(
        text.contains("after input"),
        "polling must continue past input_required, got {text}"
    );
}

#[tokio::test]
async fn a_failed_task_surfaces_the_servers_error_object() {
    let _ = ensure_test_bootstrap();

    let err = call_echo(
        "tasks_failed",
        None,
        vec![get_task(
            json!({"status": "failed", "error": {"code": -32000, "message": "boom"}}),
        )],
    )
    .await
    .expect_err("a failed task is an error");

    let msg = err.to_string();
    assert!(msg.contains("task failed"), "unexpected message: {msg}");
    assert!(
        msg.contains("boom"),
        "the server's own error object must survive: {msg}"
    );
}

#[tokio::test]
async fn a_cancelled_task_is_reported_as_cancelled_by_the_server() {
    let _ = ensure_test_bootstrap();

    let err = call_echo(
        "tasks_cancelled",
        None,
        vec![get_task(json!({"status": "cancelled"}))],
    )
    .await
    .expect_err("a cancelled task is an error");

    assert!(
        err.to_string().contains("cancelled by the server"),
        "the caller must be able to tell cancellation from failure: {err}"
    );
}

#[tokio::test]
async fn a_task_completing_with_a_non_call_tool_result_payload_is_rejected() {
    let _ = ensure_test_bootstrap();

    let err = call_echo(
        "tasks_badpayload",
        None,
        vec![completed(json!({"content": "not-a-list"}))],
    )
    .await
    .expect_err("a malformed completion payload is an error");

    assert!(
        err.to_string().contains("non-CallToolResult payload"),
        "a malformed completion must not be passed off as a result: {err}"
    );
}

#[tokio::test]
async fn a_tasks_get_that_fails_aborts_the_poll_loop() {
    let _ = ensure_test_bootstrap();

    let err = call_echo(
        "tasks_geterr",
        None,
        vec![json!({RPC_ERROR_MARKER: {"code": -32601, "message": "tasks/get unsupported"}})],
    )
    .await
    .expect_err("a tasks/get failure aborts rather than spinning");

    assert!(
        err.to_string().contains("tasks/get failed"),
        "unexpected message: {err}"
    );
}

// Why: without a deadline a server that answers `working` forever pins the
// caller in an unbounded poll loop. The TTL the server itself advertised is
// what bounds it.
#[tokio::test]
async fn a_task_whose_ttl_has_already_elapsed_times_out_instead_of_polling_forever() {
    let _ = ensure_test_bootstrap();

    let err = call_echo("tasks_ttl", Some(1), vec![working()])
        .await
        .expect_err("an elapsed ttl must end the loop");

    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("timed out") || msg.contains("Timeout"),
        "an exhausted ttl budget must surface as a timeout, got: {msg}"
    );
}
