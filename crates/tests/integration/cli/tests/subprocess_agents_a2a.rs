//! Subprocess coverage for the agent A2A client commands: registry parsing,
//! message send (plain, streaming, blocking), task retrieval, tools listing
//! against the scripted MCP stub, and the edit/delete authoring flows.

use std::path::Path;

use predicates::prelude::*;
use serde_json::json;
use systemprompt_cli_integration_tests::full_bootstrap::{
    FIXTURE_AGENT, FIXTURE_DELETE_AGENT, FIXTURE_EDIT_AGENT, command_or_skip, fixture_mcp_server,
    fixture_or_skip,
};
use systemprompt_cli_integration_tests::mcp_stub::stub_port;
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

struct StreamFrameMatcher {
    text: &'static str,
}

impl Match for StreamFrameMatcher {
    fn matches(&self, request: &Request) -> bool {
        let Ok(body) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
            return false;
        };
        let Ok(accept_headers) = request
            .headers
            .get_all("accept")
            .iter()
            .map(|value| value.to_str())
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        let accepts = accept_headers
            .into_iter()
            .flat_map(|value| value.split(',').map(str::trim))
            .collect::<Vec<_>>();
        let Ok(authorizations) = request
            .headers
            .get_all("authorization")
            .iter()
            .map(|value| value.to_str())
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        let one_session_token = authorizations
            .first()
            .is_some_and(|first| authorizations.iter().all(|value| value == first));
        body["jsonrpc"] == "2.0"
            && body["method"] == "SendStreamingMessage"
            && body["params"]["message"]["parts"] == json!([{"text": self.text}])
            && !accepts.is_empty()
            && accepts.iter().all(|value| *value == "text/event-stream")
            && !authorizations.is_empty()
            && one_session_token
            && authorizations
                .iter()
                .all(|value| value.starts_with("Bearer ") && value.len() > "Bearer ".len())
    }
}

fn spawn_server_from(build: impl FnOnce() -> Vec<Mock> + Send + 'static) -> u16 {
    spawn_server(build())
}

fn spawn_server(mocks: Vec<Mock>) -> u16 {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build mock runtime");
        runtime.block_on(async move {
            let server = MockServer::start().await;
            for mock in mocks {
                server.register(mock).await;
            }
            tx.send(server.address().port()).expect("send mock port");
            std::future::pending::<()>().await;
        });
    });
    rx.recv().expect("receive mock port")
}

fn home_cmd_or_skip(home: &Path, args: &[&str]) -> Option<assert_cmd::Command> {
    let mut cmd = command_or_skip()?;
    cmd.env("HOME", home);
    cmd.current_dir(home);
    cmd.args(args);
    Some(cmd)
}

fn completed_task_json() -> serde_json::Value {
    json!({
        "id": "task-cov-1",
        "contextId": "b8a7c0de-1111-2222-3333-444455556666",
        "status": {
            "state": "TASK_STATE_COMPLETED",
            "message": {
                "role": "ROLE_AGENT",
                "parts": [{"text": "hello from fixture"}],
                "messageId": "msg-cov-1",
                "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                "metadata": null,
                "extensions": null
            },
            "timestamp": null
        },
        "history": null,
        "artifacts": null,
        "metadata": null
    })
}

#[test]
fn registry_parses_agent_cards() {
    if fixture_or_skip().is_none() {
        return;
    }
    let port = spawn_server_from(|| {
        let body = json!({
            "data": [
                {
                    "name": "covagent",
                    "description": "Coverage agent",
                    "supportedInterfaces": [
                        {"url": "http://127.0.0.1:4777/api/v1/agents/covagent/"}
                    ],
                    "version": "1.0.0",
                    "capabilities": {
                        "streaming": true,
                        "extensions": [
                            {"uri": "systemprompt:service-status", "params": {"status": "running"}}
                        ]
                    },
                    "skills": [{"name": "echo"}]
                },
                {
                    "name": "idleagent",
                    "description": "Idle agent",
                    "supportedInterfaces": [],
                    "version": "0.1.0",
                    "capabilities": {},
                    "skills": []
                }
            ]
        });
        let mock = Mock::given(method("GET"))
            .and(path("/api/v1/agents/registry"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body));
        vec![mock]
    });
    let url = format!("http://127.0.0.1:{port}");
    for extra in [vec![], vec!["--running"], vec!["--verbose"], vec!["--json"]] {
        let Some(mut cmd) = command_or_skip() else {
            return;
        };
        if extra.contains(&"--json") {
            cmd.arg("--json");
        }
        cmd.args(["admin", "agents", "registry", "--url", &url]);
        for flag in &extra {
            if *flag != "--json" {
                cmd.arg(flag);
            }
        }
        let assert = cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("covagent"));
        if extra.contains(&"--running") {
            assert.stdout(predicate::str::contains("idleagent").not());
        }
    }
}

#[test]
fn registry_error_paths() {
    if fixture_or_skip().is_none() {
        return;
    }
    let port = spawn_server_from(|| {
        let mock = Mock::given(method("GET"))
            .and(path("/api/v1/agents/registry"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"));
        vec![mock]
    });
    let Some(mut cmd) = command_or_skip() else {
        return;
    };
    cmd.args([
        "admin",
        "agents",
        "registry",
        "--url",
        &format!("http://127.0.0.1:{port}"),
    ]);
    cmd.assert().failure();

    let Some(mut unreachable) = command_or_skip() else {
        return;
    };
    unreachable.args(["admin", "agents", "registry", "--url", "http://127.0.0.1:9"]);
    unreachable.assert().failure();
}

#[test]
fn message_non_streaming_roundtrip() {
    if fixture_or_skip().is_none() {
        return;
    }
    let home = tempfile::tempdir().expect("home");
    let agent_path = format!("/api/v1/agents/{FIXTURE_AGENT}");
    let port = spawn_server_from(move || {
        let body = json!({
            "jsonrpc": "2.0",
            "result": completed_task_json(),
            "id": "1"
        });
        let mock = Mock::given(method("POST"))
            .and(path(agent_path.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(body));
        vec![mock]
    });
    let url = format!("http://127.0.0.1:{port}");
    let Some(mut cmd) = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "ping",
            "--url",
            &url,
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello from fixture"));

    let Some(mut blocking) = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "ping",
            "--blocking",
            "--json",
            "--url",
            &url,
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    blocking
        .assert()
        .success()
        .stdout(predicate::str::contains("TASK_STATE_COMPLETED"));
}

#[test]
fn message_error_and_missing_agent() {
    if fixture_or_skip().is_none() {
        return;
    }
    let home = tempfile::tempdir().expect("home");
    let agent_path = format!("/api/v1/agents/{FIXTURE_AGENT}");
    let port = spawn_server_from(move || {
        let body = json!({
            "jsonrpc": "2.0",
            "error": {"code": -32000, "message": "agent exploded", "data": {"why": "test"}},
            "id": "1"
        });
        let mock = Mock::given(method("POST"))
            .and(path(agent_path.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(body));
        vec![mock]
    });
    let url = format!("http://127.0.0.1:{port}");
    let Some(mut cmd) = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "ping",
            "--url",
            &url,
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    cmd.assert().failure();

    let Some(mut missing) = home_cmd_or_skip(
        home.path(),
        &["admin", "agents", "message", "no_such_agent", "-m", "hi"],
    ) else {
        return;
    };
    missing.assert().failure();

    let Some(mut no_text) =
        home_cmd_or_skip(home.path(), &["admin", "agents", "message", FIXTURE_AGENT])
    else {
        return;
    };
    no_text.assert().failure();
}

#[test]
fn message_streaming_roundtrip() {
    if fixture_or_skip().is_none() {
        return;
    }
    let home = tempfile::tempdir().expect("home");
    let agent_path = format!("/api/v1/agents/{FIXTURE_AGENT}");
    let port = spawn_server_from(move || {
        let chunk = json!({
            "jsonrpc": "2.0",
            "result": {
                "kind": "status-update",
                "taskId": "task-cov-2",
                "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                "status": {
                    "state": "TASK_STATE_WORKING",
                    "message": {
                        "role": "ROLE_AGENT",
                        "parts": [{"text": "chunk one "}],
                        "messageId": "msg-s-1",
                        "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                        "metadata": null,
                        "extensions": null
                    },
                    "timestamp": null
                },
                "final": false
            },
            "id": "1"
        });
        let done = json!({
            "jsonrpc": "2.0",
            "result": {
                "kind": "status-update",
                "taskId": "task-cov-2",
                "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                "status": {
                    "state": "TASK_STATE_COMPLETED",
                    "message": {
                        "role": "ROLE_AGENT",
                        "parts": [{"text": "chunk two"}],
                        "messageId": "msg-s-2",
                        "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                        "metadata": null,
                        "extensions": null
                    },
                    "timestamp": null
                },
                "final": true
            },
            "id": "1"
        });
        let sse = format!("data: {chunk}\n\ndata: {done}\n\n");
        let mock = Mock::given(method("POST"))
            .and(path(agent_path.as_str()))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(sse, "text/event-stream"),
            );
        vec![mock]
    });
    let Some(mut cmd) = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "ping",
            "--stream",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("chunk two"));
}

#[test]
fn task_get_roundtrip() {
    if fixture_or_skip().is_none() {
        return;
    }
    let home = tempfile::tempdir().expect("home");
    let agent_path = format!("/api/v1/agents/{FIXTURE_AGENT}");
    let port = spawn_server_from(move || {
        let body = json!({
            "jsonrpc": "2.0",
            "result": completed_task_json(),
            "id": "1"
        });
        let mock = Mock::given(method("POST"))
            .and(path(agent_path.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(body));
        vec![mock]
    });
    let Some(mut cmd) = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "task",
            FIXTURE_AGENT,
            "--task-id",
            "task-cov-1",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("task-cov-1"));

    let Some(mut missing_task) =
        home_cmd_or_skip(home.path(), &["admin", "agents", "task", FIXTURE_AGENT])
    else {
        return;
    };
    missing_task.assert().failure();
}

#[test]
fn tools_lists_stub_mcp_tools() {
    if stub_port().is_none() {
        return;
    }
    let home = tempfile::tempdir().expect("home");
    let Some(mut cmd) = home_cmd_or_skip(home.path(), &["admin", "agents", "tools", FIXTURE_AGENT])
    else {
        return;
    };
    cmd.assert().success();

    let Some(mut detailed) = home_cmd_or_skip(
        home.path(),
        &[
            "--json",
            "admin",
            "agents",
            "tools",
            FIXTURE_AGENT,
            "--detailed",
        ],
        // skip-ok: the systemprompt binary is not built in this checkout
    ) else {
        return;
    };
    detailed
        .assert()
        .success()
        .stdout(predicate::str::contains("echo"));

    let Some(mut missing) = home_cmd_or_skip(home.path(), &["admin", "agents", "tools", "nope"])
    else {
        return;
    };
    missing.assert().failure();
}

#[test]
fn edit_apply_covers_field_groups() {
    if fixture_or_skip().is_none() {
        return;
    }
    let Some(mut edit) = command_or_skip() else {
        return;
    };
    edit.args([
        "admin",
        "agents",
        "edit",
        FIXTURE_EDIT_AGENT,
        "--display-name",
        "Edited Agent",
        "--description",
        "edited description",
        "--version",
        "2.0.0",
        "--icon-url",
        "https://example.com/icon.png",
        "--documentation-url",
        "https://example.com/docs",
        "--streaming",
        "false",
        "--push-notifications",
        "true",
        "--state-transition-history",
        "false",
        "--provider",
        "anthropic",
        "--model",
        "claude-sonnet-4-5",
        "--system-prompt",
        "You are edited.",
        "--mcp-server",
        fixture_mcp_server(),
        "--skill",
        "echo_skill",
        "--port",
        "4780",
        "--endpoint",
        "/api/v1/agents/covedit/",
        "--dev-only",
        "--is-primary",
    ]);
    edit.assert().success();

    let Some(mut edit2) = command_or_skip() else {
        return;
    };
    edit2.args([
        "admin",
        "agents",
        "edit",
        FIXTURE_EDIT_AGENT,
        "--remove-mcp-server",
        fixture_mcp_server(),
        "--remove-skill",
        "echo_skill",
        "--set",
        "card.description=set-edited",
        "--disable",
    ]);
    edit2.assert().success();

    let Some(mut edit3) = command_or_skip() else {
        return;
    };
    edit3.args(["admin", "agents", "edit", FIXTURE_EDIT_AGENT, "--enable"]);
    edit3.assert().success();

    let Some(mut bad_set) = command_or_skip() else {
        return;
    };
    bad_set.args([
        "admin",
        "agents",
        "edit",
        FIXTURE_EDIT_AGENT,
        "--set",
        "not_an_assignment",
    ]);
    bad_set.assert().failure();

    let Some(mut delete) = command_or_skip() else {
        return;
    };
    delete.args(["admin", "agents", "delete", FIXTURE_EDIT_AGENT, "--yes"]);
    delete.assert().success();

    let Some(mut delete_missing) = command_or_skip() else {
        return;
    };
    delete_missing.args(["admin", "agents", "delete", FIXTURE_EDIT_AGENT, "--yes"]);
    delete_missing.assert().failure();
}

#[test]
fn delete_all_and_validate() {
    if fixture_or_skip().is_none() {
        return;
    }
    let Some(mut create) = command_or_skip() else {
        return;
    };
    create.args([
        "admin",
        "agents",
        "create",
        "--name",
        "cov_created_agent",
        "--display-name",
        "Created Agent",
        "--description",
        "create coverage fixture",
        "--port",
        "4781",
    ]);
    create.assert().success();

    let Some(mut dup) = command_or_skip() else {
        return;
    };
    dup.args([
        "admin",
        "agents",
        "create",
        "--name",
        "cov_created_agent",
        "--port",
        "4782",
    ]);
    dup.assert().failure();
    let Some(mut validate) = command_or_skip() else {
        return;
    };
    validate.args(["admin", "agents", "validate"]);
    let _ = validate.assert();

    let Some(mut delete) = command_or_skip() else {
        return;
    };
    delete.args([
        "admin",
        "agents",
        "delete",
        FIXTURE_DELETE_AGENT,
        "--yes",
        "--force",
    ]);
    delete.assert().success();
}

async fn assert_stream_request(server: &MockServer, expected_text: &str) {
    let requests = server
        .received_requests()
        .await
        .expect("recorded stream request");
    let request = requests
        .iter()
        .find(|request| request.method.as_str() == "POST")
        .expect("stream POST request");
    let body: serde_json::Value =
        serde_json::from_slice(&request.body).expect("JSON-RPC request body");
    assert_eq!(
        request.url.path(),
        format!("/api/v1/agents/{FIXTURE_AGENT}")
    );
    let accept_headers = request
        .headers
        .get_all("accept")
        .iter()
        .map(|value| value.to_str())
        .collect::<Result<Vec<_>, _>>()
        .expect("Accept header values must be valid text");
    let accepts = accept_headers
        .into_iter()
        .flat_map(|value| value.split(',').map(str::trim))
        .collect::<Vec<_>>();
    assert!(
        !accepts.is_empty() && accepts.iter().all(|value| *value == "text/event-stream"),
        "all Accept values must request SSE: {:?}",
        request.headers
    );
    let authorizations = request
        .headers
        .get_all("authorization")
        .iter()
        .map(|value| value.to_str())
        .collect::<Result<Vec<_>, _>>()
        .expect("Authorization header values must be valid text");
    assert!(
        !authorizations.is_empty()
            && authorizations
                .first()
                .is_some_and(|first| authorizations.iter().all(|value| value == first))
            && authorizations
                .iter()
                .all(|value| value.starts_with("Bearer ") && value.len() > "Bearer ".len()),
        "all authorization values must carry the owned session token: {:?}",
        request.headers
    );
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["method"], "SendStreamingMessage");
    assert_eq!(body["params"]["message"]["parts"][0]["text"], expected_text);
    assert!(
        body["params"]["message"]["contextId"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "session context must be carried in the streamed message: {body}"
    );
}

fn configure_owned_web_paths() {
    let fixture = fixture_or_skip().expect("full CLI fixture requires DATABASE_URL");
    let templates = fixture.services_dir.join("web/templates");
    let assets = fixture.services_dir.join("web/assets");
    std::fs::create_dir_all(&templates).expect("create owned web templates");
    std::fs::create_dir_all(&assets).expect("create owned web assets");
    let config_path = fixture.services_dir.join("web/config.yaml");
    let mut config = std::fs::read_to_string(&config_path).expect("read owned web config");
    config.push_str(&format!(
        "\npaths:\n  templates: {}\n  assets: {}\n",
        templates.display(),
        assets.display()
    ));
    std::fs::write(config_path, config).expect("write owned web paths");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_streaming_skips_invalid_frame_then_reports_jsonrpc_error_details() {
    tokio::task::spawn_blocking(|| {
        fixture_or_skip().expect("full CLI fixture requires DATABASE_URL");
        configure_owned_web_paths();
    })
    .await
    .expect("initialize full CLI fixture");
    let home = tempfile::tempdir().expect("home");
    let server = MockServer::start().await;
    let error = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32041,
            "message": "stream rejected",
            "data": {"reason": "owned-policy"}
        },
        "id": "stream-error"
    });
    let sse = format!("data: not-json\n\ndata: {error}\n\n");
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/agents/{FIXTURE_AGENT}")))
        .and(StreamFrameMatcher {
            text: "trigger stream error",
        })
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let url = server.uri();
    let mut command = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "trigger stream error",
            "--stream",
            "--url",
            &url,
        ],
    )
    .expect("systemprompt test binary and full fixture");
    let output = tokio::task::spawn_blocking(move || command.output().expect("bounded CLI output"))
        .await
        .expect("join CLI subprocess");
    assert_stream_request(&server, "trigger stream error").await;
    assert!(!output.status.success(), "JSON-RPC stream error must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Agent returned error (-32041): stream rejected"),
        "{stderr}"
    );
    assert!(stderr.contains("owned-policy"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn message_streaming_rejects_eof_without_a_final_task() {
    tokio::task::spawn_blocking(|| {
        fixture_or_skip().expect("full CLI fixture requires DATABASE_URL");
        configure_owned_web_paths();
    })
    .await
    .expect("initialize full CLI fixture");
    let home = tempfile::tempdir().expect("home");
    let server = MockServer::start().await;
    let partial = json!({
        "jsonrpc": "2.0",
        "result": {
            "kind": "status-update",
            "taskId": "task-incomplete",
            "contextId": "b8a7c0de-1111-2222-3333-444455556666",
            "status": {
                "state": "TASK_STATE_WORKING",
                "message": {
                    "role": "ROLE_AGENT",
                    "parts": [{"text": "partial response"}],
                    "messageId": "msg-incomplete",
                    "contextId": "b8a7c0de-1111-2222-3333-444455556666",
                    "metadata": null,
                    "extensions": null
                },
                "timestamp": null
            },
            "final": false
        },
        "id": "stream-incomplete"
    });
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/agents/{FIXTURE_AGENT}")))
        .and(StreamFrameMatcher {
            text: "expect complete response",
        })
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(format!("data: {partial}\n\n"), "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let url = server.uri();
    let mut command = home_cmd_or_skip(
        home.path(),
        &[
            "admin",
            "agents",
            "message",
            FIXTURE_AGENT,
            "-m",
            "expect complete response",
            "--stream",
            "--url",
            &url,
        ],
    )
    .expect("systemprompt test binary and full fixture");
    let output = tokio::task::spawn_blocking(move || command.output().expect("bounded CLI output"))
        .await
        .expect("join CLI subprocess");
    assert_stream_request(&server, "expect complete response").await;
    assert!(!output.status.success(), "unterminated stream must fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("partial response"), "{stdout}");
    assert!(
        stderr.contains("Stream ended without final task"),
        "{stderr}"
    );
}
