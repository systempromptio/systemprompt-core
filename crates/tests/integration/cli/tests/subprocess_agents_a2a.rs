//! Subprocess coverage for the agent A2A client commands: registry parsing,
//! message and task argument failures, and the edit/delete authoring flows.

use std::path::Path;

use predicates::prelude::*;
use serde_json::json;
use systemprompt_cli_integration_tests::full_bootstrap::{
    FIXTURE_AGENT, FIXTURE_DELETE_AGENT, FIXTURE_EDIT_AGENT, cli_command, fixture_mcp_server,
    full_fixture,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn home_cmd(home: &Path, args: &[&str]) -> assert_cmd::Command {
    let mut cmd = cli_command();
    cmd.env("HOME", home);
    cmd.current_dir(home);
    cmd.args(args);
    cmd
}

#[test]
fn registry_parses_agent_cards() {
    full_fixture();
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
        let mut cmd = cli_command();
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
    full_fixture();
    let port = spawn_server_from(|| {
        let mock = Mock::given(method("GET"))
            .and(path("/api/v1/agents/registry"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"));
        vec![mock]
    });
    let mut cmd = cli_command();
    cmd.args([
        "admin",
        "agents",
        "registry",
        "--url",
        &format!("http://127.0.0.1:{port}"),
    ]);
    cmd.assert().failure();

    let mut unreachable = cli_command();
    unreachable.args(["admin", "agents", "registry", "--url", "http://127.0.0.1:9"]);
    unreachable.assert().failure();
}


#[test]
fn message_to_a_missing_agent_or_without_text_fails() {
    full_fixture();
    let home = tempfile::tempdir().expect("home");
    let mut missing = home_cmd(
        home.path(),
        &["admin", "agents", "message", "no_such_agent", "-m", "hi"],
    );
    missing.assert().failure();

    let mut no_text = home_cmd(home.path(), &["admin", "agents", "message", FIXTURE_AGENT]);
    no_text.assert().failure();
}


#[test]
fn task_without_a_task_id_fails() {
    full_fixture();
    let home = tempfile::tempdir().expect("home");
    let mut missing_task = home_cmd(home.path(), &["admin", "agents", "task", FIXTURE_AGENT]);
    missing_task.assert().failure();
}


#[test]
fn edit_apply_covers_field_groups() {
    full_fixture();
    let mut edit = cli_command();
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
        "--extended-agent-card",
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

    let mut edit2 = cli_command();
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

    let mut edit3 = cli_command();
    edit3.args(["admin", "agents", "edit", FIXTURE_EDIT_AGENT, "--enable"]);
    edit3.assert().success();

    let mut bad_set = cli_command();
    bad_set.args([
        "admin",
        "agents",
        "edit",
        FIXTURE_EDIT_AGENT,
        "--set",
        "not_an_assignment",
    ]);
    bad_set.assert().failure();

    let mut delete = cli_command();
    delete.args(["admin", "agents", "delete", FIXTURE_EDIT_AGENT, "--yes"]);
    delete.assert().success();

    let mut delete_missing = cli_command();
    delete_missing.args(["admin", "agents", "delete", FIXTURE_EDIT_AGENT, "--yes"]);
    delete_missing.assert().failure();
}

#[test]
fn delete_all_and_validate() {
    full_fixture();
    let mut create = cli_command();
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

    let mut dup = cli_command();
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
    let mut validate = cli_command();
    validate.args(["admin", "agents", "validate"]);
    let _ = validate.assert();

    let mut delete = cli_command();
    delete.args(["admin", "agents", "delete", FIXTURE_DELETE_AGENT, "--yes"]);
    delete.assert().success();
}
