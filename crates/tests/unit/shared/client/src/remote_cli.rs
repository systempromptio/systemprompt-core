//! Tests for `RemoteCliExecutor`: SSE streaming of remote CLI output into a
//! capturing `OutputSink`, exit-code propagation, and request shaping.

use std::io;

use systemprompt_client::{OutputSink, RemoteCliExecutor, RemoteCliRequest};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct CapturingSink {
    stdout: String,
    stderr: String,
    errors: Vec<String>,
}

impl OutputSink for CapturingSink {
    fn stdout_chunk(&mut self, data: &str) -> io::Result<()> {
        self.stdout.push_str(data);
        Ok(())
    }

    fn stderr_chunk(&mut self, data: &str) -> io::Result<()> {
        self.stderr.push_str(data);
        Ok(())
    }

    fn error_message(&mut self, message: &str) {
        self.errors.push(message.to_owned());
    }
}

fn sse_body(events: &[&str]) -> String {
    events
        .iter()
        .map(|data| format!("event: cli\ndata: {data}\n\n"))
        .collect()
}

fn sse_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "text/event-stream")
}

#[tokio::test]
async fn execute_streams_output_and_returns_exit_code() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"type":"started","pid":42}"#,
        r#"{"type":"stdout","data":"hello "}"#,
        r#"{"type":"stdout","data":"world\n"}"#,
        r#"{"type":"stderr","data":"warn\n"}"#,
        r#"{"type":"exit_code","code":3}"#,
    ]);

    Mock::given(method("POST"))
        .and(path("/api/v1/admin/cli"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let executor = RemoteCliExecutor::new(&server.uri(), 30).expect("build executor");
    let mut sink = CapturingSink::default();
    let args = vec!["infra".to_owned(), "db".to_owned(), "status".to_owned()];

    let exit_code = executor
        .execute(
            RemoteCliRequest {
                token: "test-token",
                context: "",
                args: &args,
            },
            &mut sink,
        )
        .await
        .expect("execute");

    assert_eq!(exit_code, 3);
    assert_eq!(sink.stdout, "hello world\n");
    assert_eq!(sink.stderr, "warn\n");
    assert!(sink.errors.is_empty());
}

#[tokio::test]
async fn execute_sends_context_header_and_body() {
    let server = MockServer::start().await;
    let args = vec!["core".to_owned(), "content".to_owned()];
    let expected_body = serde_json::json!({
        "args": ["core", "content"],
        "timeout_secs": 60,
        "context_id": "6f7d9a40-1f2b-4c3d-8e5f-0a1b2c3d4e5f",
    });

    Mock::given(method("POST"))
        .and(path("/api/v1/admin/cli"))
        .and(header(
            "x-context-id",
            "6f7d9a40-1f2b-4c3d-8e5f-0a1b2c3d4e5f",
        ))
        .and(body_json(&expected_body))
        .respond_with(sse_response(sse_body(&[
            r#"{"type":"exit_code","code":0}"#,
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let executor = RemoteCliExecutor::new(&server.uri(), 60).expect("build executor");
    let mut sink = CapturingSink::default();

    let exit_code = executor
        .execute(
            RemoteCliRequest {
                token: "test-token",
                context: "6f7d9a40-1f2b-4c3d-8e5f-0a1b2c3d4e5f",
                args: &args,
            },
            &mut sink,
        )
        .await
        .expect("execute");

    assert_eq!(exit_code, 0);
}

#[tokio::test]
async fn execute_forwards_error_events_and_keeps_exit_code() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"type":"error","message":"remote command failed"}"#,
        r#"{"type":"exit_code","code":2}"#,
    ]);

    Mock::given(method("POST"))
        .and(path("/api/v1/admin/cli"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let executor = RemoteCliExecutor::new(&server.uri(), 30).expect("build executor");
    let mut sink = CapturingSink::default();
    let args = vec!["status".to_owned()];

    let exit_code = executor
        .execute(
            RemoteCliRequest {
                token: "test-token",
                context: "",
                args: &args,
            },
            &mut sink,
        )
        .await
        .expect("execute");

    assert_eq!(exit_code, 2);
    assert_eq!(sink.errors, vec!["remote command failed".to_owned()]);
}

#[tokio::test]
async fn execute_skips_unparseable_and_non_cli_events() {
    let server = MockServer::start().await;
    let body = format!(
        "event: other\ndata: ignored\n\n{}",
        sse_body(&["not-json", r#"{"type":"exit_code","code":7}"#])
    );

    Mock::given(method("POST"))
        .and(path("/api/v1/admin/cli"))
        .respond_with(sse_response(body))
        .mount(&server)
        .await;

    let executor = RemoteCliExecutor::new(&server.uri(), 30).expect("build executor");
    let mut sink = CapturingSink::default();
    let args = vec!["status".to_owned()];

    let exit_code = executor
        .execute(
            RemoteCliRequest {
                token: "test-token",
                context: "",
                args: &args,
            },
            &mut sink,
        )
        .await
        .expect("execute");

    assert_eq!(exit_code, 7);
    assert!(sink.stdout.is_empty());
    assert!(sink.errors.is_empty());
}

#[tokio::test]
async fn execute_reports_connection_error_as_exit_code_one() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/admin/cli"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let executor = RemoteCliExecutor::new(&server.uri(), 30).expect("build executor");
    let mut sink = CapturingSink::default();
    let args = vec!["status".to_owned()];

    let exit_code = executor
        .execute(
            RemoteCliRequest {
                token: "test-token",
                context: "",
                args: &args,
            },
            &mut sink,
        )
        .await
        .expect("execute");

    assert_eq!(exit_code, 1);
    assert_eq!(sink.errors.len(), 1);
    assert!(sink.errors[0].starts_with("Connection error: "));
}
