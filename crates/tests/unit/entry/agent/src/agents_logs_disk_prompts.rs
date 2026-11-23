//! Disk-mode `admin agents logs` tests, driven through the dispatcher with a
//! temp log directory and a `ScriptedPrompter` for interactive selection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_agent_entry::cli::agents;
use systemprompt_cli::CommandContext;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_extension::cli::{CliConfig, OutputFormat};

use crate::common::{agents_host, parse_agents, with_boot};


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
    with_boot(config).with_prompter(Box::new(ScriptedPrompter::new(
        answers.iter().map(|s| (*s).to_owned()),
    )))
}

fn seed_logs(dir: &Path) {
    for (name, body) in [
        ("agent-orion.log", "INFO orion booted\nERROR orion failed\n"),
        ("agent-vega.log", "INFO vega booted\n"),
    ] {
        std::fs::write(dir.join(name), body).unwrap();
    }
}

#[tokio::test]
async fn agent_logs_disk_lists_files_when_no_agent_given() {
    let dir = tempfile::tempdir().unwrap();
    seed_logs(dir.path());
    agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &agents_host(&ctx(false, &[])),
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
        &agents_host(&ctx(false, &[])),
    )
    .await
    .unwrap();
    agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", dir_arg]),
        &agents_host(&ctx(true, &["0"])),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn agent_logs_disk_error_branches() {
    let missing = agents::execute(
        parse_agents(&["logs", "--disk", "--logs-dir", "/no/such/dir/cov"]),
        &agents_host(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(format!("{missing:#}").contains("does not exist"));

    let follow_missing = agents::execute(
        parse_agents(&["logs", "--follow", "--logs-dir", "/no/such/dir/cov"]),
        &agents_host(&ctx(false, &[])),
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
        &agents_host(&ctx(true, &[])),
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
        &agents_host(&ctx(false, &[])),
    )
    .await;
    flag_required.unwrap();
}
