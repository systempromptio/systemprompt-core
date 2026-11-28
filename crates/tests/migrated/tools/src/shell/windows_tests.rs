//! The shell tool on a Windows host: routing through cmd and `PowerShell`,
//! scripts in directories with spaces, output and exit status, arguments,
//! and the permission question under `PowerShell`.

use std::path::Path;

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, OutputLimits, ShellConfig, ShellKind, ShellSpec, ShellTool,
};

use crate::support::{Called, Project};

fn tool(spec: ShellSpec) -> ShellTool {
    ShellTool::with_spec(
        spec,
        ShellConfig::default(),
        OutputLimits::default(),
        BackgroundJobs::new(),
    )
}

fn cmd() -> ShellTool {
    tool(ShellSpec::resolve(Path::new("cmd")).unwrap())
}

fn powershell() -> ShellTool {
    tool(ShellSpec::resolve(Path::new("powershell")).unwrap())
}

async fn run(project: &Project, shell: &ShellTool, command: &str) -> Called {
    project.call(shell, json!({"command": command})).await
}

#[tokio::test]
async fn a_command_runs_through_cmd_and_through_powershell() {
    let project = Project::new();
    for shell in [cmd(), powershell()] {
        let out = run(&project, &shell, "echo routed").await.ok();
        assert!(out.output.contains("routed"), "{}", out.output);
        assert_eq!(out.metadata["exit_code"], 0);
    }
}

#[tokio::test]
async fn a_cmd_script_in_a_directory_with_spaces_runs() {
    let project = Project::new();
    let script = project.put("dir with space/run me.cmd", b"@echo from-script\r\n");
    let out = run(&project, &cmd(), &format!("\"{}\"", script.display()))
        .await
        .ok();
    assert!(out.output.contains("from-script"), "{}", out.output);
}

#[tokio::test]
async fn output_arrives_in_order_with_the_exit_code() {
    let project = Project::new();
    let out = run(&project, &cmd(), "echo one& echo two& exit /b 3")
        .await
        .ok();
    let lines: Vec<&str> = out
        .output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(lines, ["one", "two", "[Exit code: 3]"], "{}", out.output);
    assert_eq!(out.metadata["exit_code"], 3);
    let silent = run(&project, &cmd(), "exit /b 0").await.ok();
    assert_eq!(silent.output.trim(), "(no output)");
    assert_eq!(silent.metadata["exit_code"], 0);
}

#[tokio::test]
async fn stdout_still_arrives_after_stderr_is_closed() {
    let project = Project::new();
    let out = run(&project, &cmd(), "echo kept 2>nul").await.ok();
    assert!(out.output.contains("kept"), "{}", out.output);
}

#[tokio::test]
async fn quoted_arguments_reach_the_command_intact() {
    let project = Project::new();
    let out = run(
        &project,
        &powershell(),
        "Write-Output 'a b'; Write-Output 'c\"d'; Write-Output ''",
    )
    .await
    .ok();
    let lines: Vec<&str> = out.output.lines().collect();
    assert_eq!(lines.first().copied(), Some("a b"), "{}", out.output);
    assert_eq!(lines.get(1).copied(), Some("c\"d"), "{}", out.output);
}

#[tokio::test]
async fn each_powershell_command_is_asked_about_with_its_reusable_prefix() {
    let project = Project::new();
    let called = run(&project, &powershell(), "Get-ChildItem; Write-Output ok").await;
    let asked = called.gate.asked();
    called.ok();
    let shell = asked
        .iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(shell.patterns.len(), 2, "{:?}", shell.patterns);
    assert_eq!(shell.always.len(), 2, "{:?}", shell.always);
    for (pattern, always) in shell.patterns.iter().zip(&shell.always) {
        let stem = always.trim_end_matches(" *").to_lowercase();
        assert!(
            pattern.to_lowercase().starts_with(&stem),
            "{pattern} / {always}"
        );
    }
}

#[test]
fn a_bare_name_gains_its_executable_extension() {
    let found = ShellSpec::resolve(Path::new("cmd")).unwrap();
    let extension = found
        .path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());
    assert_eq!(
        extension.as_deref(),
        Some("exe"),
        "{}",
        found.path.display()
    );
}

#[test]
#[ignore = "parity ledger: a bare bash name is looked up on the search path, not beside git"]
fn a_bare_bash_name_resolves_to_git_bash() {
    let found = ShellSpec::resolve(Path::new("bash")).unwrap();
    assert_eq!(found.kind, ShellKind::GitBash, "{}", found.path.display());
    let lower = found.path.to_string_lossy().to_lowercase();
    assert!(!lower.contains("system32"), "{lower}");
}
