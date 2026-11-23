//! Disk-mode `admin agents logs` tests, driven through the dispatcher with a
//! temp log directory and a `ScriptedPrompter` for interactive selection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_agent_entry::cli::agents;
use systemprompt_cli::CommandContext;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_extension::cli::{CliConfig, OutputFormat};

use crate::common::{agents_host, logs_dir_in_root, parse_agents, with_boot};

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
    let dir = logs_dir_in_root(&ctx(false, &[])).await;
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
    let dir = logs_dir_in_root(&ctx(false, &[])).await;
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
    assert!(
        format!("{missing:#}").contains("must name a directory inside"),
        "{missing:#}"
    );

    let follow_missing = agents::execute(
        parse_agents(&["logs", "--follow", "--logs-dir", "/no/such/dir/cov"]),
        &agents_host(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{follow_missing:#}").contains("must name a directory inside"),
        "{follow_missing:#}"
    );

    let empty = logs_dir_in_root(&ctx(false, &[])).await;
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

#[tokio::test]
async fn agent_logs_dir_outside_the_logs_root_is_refused() {
    logs_dir_in_root(&ctx(false, &[])).await;
    let outside = tempfile::tempdir().unwrap();
    seed_logs(outside.path());
    let error = agents::execute(
        parse_agents(&[
            "logs",
            "orion",
            "--disk",
            "--logs-dir",
            outside.path().to_str().unwrap(),
        ]),
        &agents_host(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("resolves outside"),
        "{error:#}"
    );
}

#[tokio::test]
async fn agent_logs_name_that_climbs_out_of_the_logs_dir_is_refused() {
    let inside = logs_dir_in_root(&ctx(false, &[])).await;
    let root = inside.path().parent().unwrap().to_path_buf();
    let escape = format!("escape-{}", uuid::Uuid::new_v4().simple());
    std::fs::write(root.join(format!("{escape}.log")), "INFO secret\n").unwrap();
    let name = format!("../{escape}");

    let error = agents::execute(
        parse_agents(&[
            "logs",
            &name,
            "--disk",
            "--logs-dir",
            inside.path().to_str().unwrap(),
        ]),
        &agents_host(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    std::fs::remove_file(root.join(format!("{escape}.log"))).unwrap();
    assert!(
        format!("{error:#}").contains("not a single path segment"),
        "{error:#}"
    );
}
