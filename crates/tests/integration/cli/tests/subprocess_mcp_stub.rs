//! Subprocess tests for `plugins mcp call` / `plugins mcp tools` against the
//! scripted streamable-HTTP MCP stub started by [`mcp_stub`], which registers
//! `fixture_mcp` as a running service so port resolution succeeds and the
//! real client/render paths execute.

use predicates::prelude::*;
use systemprompt_cli_integration_tests::full_bootstrap::command;
use systemprompt_cli_integration_tests::mcp_stub::stub_port;

fn stub_command() -> Option<assert_cmd::Command> {
    stub_port()?;
    command()
}

#[test]
fn mcp_call_echo_succeeds() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args([
        "plugins",
        "mcp",
        "call",
        "fixture_mcp",
        "echo",
        "-a",
        r#"{"message":"hello"}"#,
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stub output"));
}

#[test]
fn mcp_call_echo_json_output() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args([
        "--json",
        "plugins",
        "mcp",
        "call",
        "fixture_mcp",
        "echo",
        "-a",
        r#"{"message":"hello"}"#,
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stub output"))
        .stdout(predicate::str::contains("execution_time_ms"));
}

#[test]
fn mcp_call_without_args_payload() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call", "fixture_mcp", "echo"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("stub output"));
}

#[test]
fn mcp_call_tool_error_maps_to_failure() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call", "fixture_mcp", "boom"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("boom exploded").or(predicate::str::contains("is_error")));
}

#[test]
fn mcp_call_rejected_tool_maps_to_failure() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call", "fixture_mcp", "reject"]);
    cmd.assert().failure();
}

#[test]
fn mcp_call_invalid_json_args_fails() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args([
        "plugins",
        "mcp",
        "call",
        "fixture_mcp",
        "echo",
        "-a",
        "{not json",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid JSON"));
}

#[test]
fn mcp_call_unknown_server_fails() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call", "no_such_server", "echo"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn mcp_call_missing_server_non_interactive_fails() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call"]);
    cmd.assert().failure();
}

#[test]
fn mcp_call_missing_tool_non_interactive_fails() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "call", "fixture_mcp"]);
    cmd.assert().failure();
}

#[test]
fn mcp_tools_lists_stub_tools() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "tools"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("echo"));
}

#[test]
fn mcp_tools_filtered_by_server_with_schema() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args([
        "plugins",
        "mcp",
        "tools",
        "--server",
        "fixture_mcp",
        "--schema",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("echo"));
}

#[test]
fn mcp_tools_json_detailed() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["--json", "plugins", "mcp", "tools", "--detailed"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("echo"));
}

#[test]
fn mcp_tools_unknown_server_fails() {
    let Some(mut cmd) = stub_command() else { return };
    cmd.args(["plugins", "mcp", "tools", "--server", "no_such_server"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not running"));
}
