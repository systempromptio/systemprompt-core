//! Disk-mode `logs` command tests for `plugins mcp logs` and
//! `admin agents logs`, driven through the dispatchers with a temp log
//! directory and a `ScriptedPrompter` for interactive selection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use clap::Parser;
use systemprompt_cli::admin::agents::{self, AgentsCommands};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

#[derive(Debug, Parser)]
struct McpHarness {
    #[command(subcommand)]
    cmd: McpCommands,
}

#[derive(Debug, Parser)]
struct AgentsHarness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

fn parse_mcp(args: &[&str]) -> McpCommands {
    McpHarness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn parse_agents(args: &[&str]) -> AgentsCommands {
    AgentsHarness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx(interactive: bool, answers: &[&str]) -> CommandContext {
    let config = if interactive {
        CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true)
            .with_output_format(OutputFormat::Json)
    } else {
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json)
    };
    CommandContext::new(config, EnvOverrides::default()).with_prompter(Box::new(
        ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned())),
    ))
}

fn seed_logs(dir: &Path) {
    for (name, body) in [
        (
            "mcp-alpha.log",
            "INFO alpha starting\nWARN alpha slow\nERROR alpha broke\n",
        ),
        ("mcp-beta.log", "INFO beta starting\nDEBUG beta detail\n"),
        ("agent-orion.log", "INFO orion booted\nERROR orion failed\n"),
        ("agent-vega.log", "INFO vega booted\n"),
    ] {
        std::fs::write(dir.join(name), body).unwrap();
    }
}

#[tokio::test]
async fn mcp_logs_disk_lists_files_when_no_server_given() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    mcp::execute(
        parse_mcp(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &ctx(false, &[]),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_reads_named_server_with_level_filter() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    let dir_arg = dir.path().to_str().unwrap();
    let ctx = ctx(false, &[]);

    mcp::execute(
        parse_mcp(&["logs", "alpha", "--disk", "--logs-dir", dir_arg]),
        &ctx,
    )
    .await
    .unwrap();
    mcp::execute(
        parse_mcp(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            dir_arg,
            "--level",
            "warn",
            "-n",
            "10",
        ]),
        &ctx,
    )
    .await
    .unwrap();
    mcp::execute(
        parse_mcp(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            dir_arg,
            "--level",
            "error",
        ]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_prompts_for_selection_when_interactive() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    mcp::execute(
        parse_mcp(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &ctx(true, &["1"]),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_error_branches() {
    let missing = mcp::execute(
        parse_mcp(&["logs", "--disk", "--logs-dir", "/no/such/dir/cov"]),
        &ctx(false, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{missing:#}").contains("does not exist"));

    let follow_missing = mcp::execute(
        parse_mcp(&["logs", "--follow", "--logs-dir", "/no/such/dir/cov"]),
        &ctx(false, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{follow_missing:#}").contains("does not exist"));

    let empty = tempfile::tempdir().unwrap();
    let no_files = mcp::execute(
        parse_mcp(&[
            "logs",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &ctx(true, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{no_files:#}").contains("No MCP log files"));

    let unknown = mcp::execute(
        parse_mcp(&[
            "logs",
            "nope",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &ctx(false, &[]),
    )
    .await
    .unwrap_err();
    assert!(!format!("{unknown:#}").is_empty());
}

#[tokio::test]
async fn agent_logs_disk_lists_files_when_no_agent_given() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &ctx(false, &[]),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn agent_logs_disk_reads_named_and_prompted_agent() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    let dir_arg = dir.path().to_str().unwrap();

    agents::execute(
        parse_agents(&["logs", "orion", "--disk", "--logs-dir", dir_arg, "-n", "5"]),
        &ctx(false, &[]),
    )
    .await
    .unwrap();
    agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", dir_arg]),
        &ctx(true, &["0"]),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn agent_logs_disk_error_branches() {
    let missing = agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", "/no/such/dir/cov"]),
        &ctx(false, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{missing:#}").contains("does not exist"));

    let follow_missing = agents::execute(
        parse_agents(&["logs", "--follow", "--logs-dir", "/no/such/dir/cov"]),
        &ctx(false, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{follow_missing:#}").contains("does not exist"));

    let empty = tempfile::tempdir().unwrap();
    let no_files = agents::execute(
        parse_agents(&[
            "logs",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &ctx(true, &[]),
    )
    .await
    .unwrap_err();
    assert!(format!("{no_files:#}").contains("No agent log files"));

    let flag_required = agents::execute(
        parse_agents(&[
            "logs",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &ctx(false, &[]),
    )
    .await;
    flag_required.unwrap();
}
