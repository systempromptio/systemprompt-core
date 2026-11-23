//! Disk-mode `plugins mcp logs` tests, driven through the dispatcher with a
//! temp log directory and a `ScriptedPrompter` for interactive selection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::CommandContext;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_mcp_entry::cli as mcp;

use crate::common::{logs_dir_in_root, logs_root, member, parse, with_boot};

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
        (
            "mcp-alpha.log",
            "INFO alpha starting\nWARN alpha slow\nERROR alpha broke\n",
        ),
        ("mcp-beta.log", "INFO beta starting\nDEBUG beta detail\n"),
    ] {
        std::fs::write(dir.join(name), body).unwrap();
    }
}

#[tokio::test]
async fn mcp_logs_disk_lists_files_when_no_server_given() {
    let dir = logs_dir_in_root(&ctx(false, &[])).await;
    seed_logs(dir.path());
    mcp::execute(
        parse(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_reads_named_server_with_level_filter() {
    let dir = logs_dir_in_root(&ctx(false, &[])).await;
    seed_logs(dir.path());
    let dir_arg = dir.path().to_str().unwrap();
    let ctx = ctx(false, &[]);

    mcp::execute(
        parse(&["logs", "alpha", "--disk", "--logs-dir", dir_arg]),
        &member(&ctx),
    )
    .await
    .unwrap();
    mcp::execute(
        parse(&[
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
        &member(&ctx),
    )
    .await
    .unwrap();
    mcp::execute(
        parse(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            dir_arg,
            "--level",
            "error",
        ]),
        &member(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_prompts_for_selection_when_interactive() {
    let dir = logs_dir_in_root(&ctx(false, &[])).await;
    seed_logs(dir.path());
    mcp::execute(
        parse(&["logs", "--disk", "--logs-dir", dir.path().to_str().unwrap()]),
        &member(&ctx(true, &["1"])),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mcp_logs_disk_error_branches() {
    let missing = mcp::execute(
        parse(&["logs", "--disk", "--logs-dir", "/no/such/dir/cov"]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{missing:#}").contains("must name a directory inside"),
        "{missing:#}"
    );

    let follow_missing = mcp::execute(
        parse(&["logs", "--follow", "--logs-dir", "/no/such/dir/cov"]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{follow_missing:#}").contains("must name a directory inside"),
        "{follow_missing:#}"
    );

    let empty = logs_dir_in_root(&ctx(false, &[])).await;
    let no_files = mcp::execute(
        parse(&[
            "logs",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &member(&ctx(true, &[])),
    )
    .await
    .unwrap_err();
    assert!(format!("{no_files:#}").contains("No MCP log files"));

    let unknown = mcp::execute(
        parse(&[
            "logs",
            "nope",
            "--disk",
            "--logs-dir",
            empty.path().to_str().unwrap(),
        ]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(!format!("{unknown:#}").is_empty());
}

#[tokio::test]
async fn mcp_logs_dir_outside_the_logs_root_is_refused() {
    logs_root(&ctx(false, &[])).await;
    let outside = tempfile::tempdir().unwrap();
    seed_logs(outside.path());
    let error = mcp::execute(
        parse(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            outside.path().to_str().unwrap(),
        ]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("resolves outside"),
        "{error:#}"
    );

    let error = mcp::execute(
        parse(&["logs", "alpha", "--disk", "--logs-dir", "../.."]),
        &member(&ctx(false, &[])),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("resolves outside"),
        "{error:#}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn mcp_logs_dir_symlinked_out_of_the_logs_root_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    seed_logs(outside.path());
    let ctx = ctx(false, &[]);
    let root = logs_root(&ctx).await;
    let link = root.join(format!("escape-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    let error = mcp::execute(
        parse(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            link.file_name().unwrap().to_str().unwrap(),
        ]),
        &member(&ctx),
    )
    .await
    .unwrap_err();
    std::fs::remove_file(&link).unwrap();
    assert!(
        format!("{error:#}").contains("resolves outside"),
        "{error:#}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_log_file_symlinked_out_of_the_logs_dir_is_not_read() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.log"), "SECRET\n").unwrap();
    let ctx = ctx(false, &[]);
    let dir = logs_dir_in_root(&ctx).await;
    std::os::unix::fs::symlink(
        outside.path().join("secret.log"),
        dir.path().join("mcp-alpha.log"),
    )
    .unwrap();
    let error = mcp::execute(
        parse(&[
            "logs",
            "alpha",
            "--disk",
            "--logs-dir",
            dir.path().to_str().unwrap(),
        ]),
        &member(&ctx),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("resolves outside"),
        "{error:#}"
    );
}
