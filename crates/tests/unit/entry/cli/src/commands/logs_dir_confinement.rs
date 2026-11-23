//! `admin agents logs` and `plugins mcp logs` read only beneath the
//! profile's logs root: a `--logs-dir` outside it, a log name that is more
//! than one path segment, and a log file linked out of the root are refused.

#![cfg(unix)]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::{self, AgentsCommands};
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[derive(Debug, Parser)]
struct AgentsHarness {
    #[command(subcommand)]
    command: AgentsCommands,
}

#[derive(Debug, Parser)]
struct McpHarness {
    #[command(subcommand)]
    command: McpCommands,
}

fn agents_cmd(args: &[&str]) -> AgentsCommands {
    AgentsHarness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .expect("parse agents logs")
        .command
}

fn mcp_cmd(args: &[&str]) -> McpCommands {
    McpHarness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .expect("parse mcp logs")
        .command
}

fn context() -> CommandContext {
    crate::boot_ctx::new_with_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn logs_root() -> PathBuf {
    let root = ensure_test_bootstrap().app_paths.system().logs();
    std::fs::create_dir_all(&root).expect("logs root");
    root
}

fn refused(error: &anyhow::Error) -> bool {
    let rendered = format!("{error:#}");
    rendered.contains("must name a directory inside")
        || rendered.contains("resolves outside")
        || rendered.contains("not a single path segment")
}

#[tokio::test]
async fn agent_logs_refuse_a_logs_dir_outside_the_logs_root() {
    let outside = tempfile::tempdir().expect("outside dir");
    std::fs::write(outside.path().join("agent-orion.log"), "INFO secret\n").unwrap();

    let error = agents::execute(
        agents_cmd(&[
            "logs",
            "orion",
            "--disk",
            "--logs-dir",
            outside.path().to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect_err("a --logs-dir outside the logs root is refused");
    assert!(refused(&error), "{error:#}");
}

#[tokio::test]
async fn mcp_logs_refuse_a_logs_dir_outside_the_logs_root() {
    let outside = tempfile::tempdir().expect("outside dir");
    std::fs::write(outside.path().join("mcp-orion.log"), "INFO secret\n").unwrap();

    let error = mcp::execute(
        mcp_cmd(&[
            "logs",
            "orion",
            "--disk",
            "--logs-dir",
            outside.path().to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect_err("a --logs-dir outside the logs root is refused");
    assert!(refused(&error), "{error:#}");
}

#[tokio::test]
async fn agent_logs_refuse_a_name_that_climbs_out_of_the_logs_dir() {
    let root = logs_root();
    let inside = tempfile::tempdir_in(&root).expect("dir inside the logs root");
    let escape = format!("escape-{}", uuid::Uuid::new_v4().simple());
    std::fs::write(root.join(format!("{escape}.log")), "INFO secret\n").unwrap();
    let name = format!("../{escape}");

    let error = agents::execute(
        agents_cmd(&[
            "logs",
            &name,
            "--disk",
            "--logs-dir",
            inside.path().to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect_err("a multi-segment log name is refused");
    assert!(refused(&error), "{error:#}");
}

#[tokio::test]
async fn mcp_logs_refuse_a_log_file_linked_out_of_the_logs_root() {
    let root = logs_root();
    let inside = tempfile::tempdir_in(&root).expect("dir inside the logs root");
    let outside = tempfile::tempdir().expect("outside dir");
    let target = outside.path().join("secret.log");
    std::fs::write(&target, "INFO secret\n").unwrap();
    std::os::unix::fs::symlink(&target, inside.path().join("mcp-leak.log")).unwrap();

    let error = mcp::execute(
        mcp_cmd(&[
            "logs",
            "leak",
            "--disk",
            "--logs-dir",
            inside.path().to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect_err("a log file linked out of the logs root is refused");
    assert!(refused(&error), "{error:#}");
}
