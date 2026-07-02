//! Subprocess tests for the `build` command group covering argument
//! validation and project-root discovery errors, plus the no-extensions
//! success path. Never runs an actual workspace build.

use predicates::prelude::*;
use systemprompt_cli_integration_tests::full_bootstrap::command;

fn empty_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create empty working dir")
}

fn project_root_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create project root dir");
    std::fs::create_dir_all(dir.path().join(".systemprompt")).expect("mkdir .systemprompt");
    std::fs::create_dir_all(dir.path().join("services")).expect("mkdir services");
    dir
}

#[test]
fn build_without_subcommand_fails() {
    let Some(mut cmd) = command() else { return };
    cmd.arg("build");
    cmd.assert().failure();
}

#[test]
fn build_core_outside_project_root_fails() {
    let Some(mut cmd) = command() else { return };
    let dir = empty_dir();
    cmd.current_dir(dir.path());
    cmd.args(["build", "core"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to build core"));
}

#[test]
fn build_core_release_offline_outside_project_root_fails() {
    let Some(mut cmd) = command() else { return };
    let dir = empty_dir();
    cmd.current_dir(dir.path());
    cmd.args(["build", "core", "--release", "--offline"]);
    cmd.assert().failure();
}

#[test]
fn build_mcp_outside_project_root_fails() {
    let Some(mut cmd) = command() else { return };
    let dir = empty_dir();
    cmd.current_dir(dir.path());
    cmd.args(["build", "mcp"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to build MCP extensions"));
}

#[test]
fn build_mcp_with_no_extensions_reports_empty() {
    let Some(mut cmd) = command() else { return };
    let dir = project_root_dir();
    cmd.current_dir(dir.path());
    cmd.args(["build", "mcp"]);
    cmd.assert().success();
}

#[test]
fn build_mcp_with_no_extensions_json() {
    let Some(mut cmd) = command() else { return };
    let dir = project_root_dir();
    cmd.current_dir(dir.path());
    cmd.args(["--json", "build", "mcp", "--release"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("build_type"));
}

#[test]
fn build_core_rejects_unknown_flag() {
    let Some(mut cmd) = command() else { return };
    cmd.args(["build", "core", "--no-such-flag"]);
    cmd.assert().failure();
}
