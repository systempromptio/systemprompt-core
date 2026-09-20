#![cfg(unix)]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::plugins::mcp::{self, McpCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

const HELPER: &str = "commands::mcp_logs_follow_lifecycle::mcp_logs_follow_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: McpCommands,
}

fn parse(args: &[&str]) -> McpCommands {
    Harness::try_parse_from(std::iter::once("mcp").chain(args.iter().copied()))
        .expect("parse MCP logs command")
        .command
}

fn context() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[tokio::test]
#[ignore = "re-executed by mcp_follow_uses_the_resolved_server_log_and_surfaces_tail_failure"]
async fn mcp_logs_follow_helper() {
    let fixture = tempfile::tempdir().expect("MCP log fixture");
    let logs = fixture.path().join("logs");
    let bin = fixture.path().join("bin");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let log = logs.join("mcp-atlas.log");
    std::fs::write(&log, "INFO atlas ready\n").unwrap();
    let capture = fixture.path().join("tail-args");
    let tail = bin.join("tail");
    std::fs::write(
        &tail,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TAIL_CAPTURE\"\nexit \"${TAIL_EXIT:-0}\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&tail).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&tail, permissions).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("TAIL_CAPTURE", &capture);
        std::env::remove_var("TAIL_EXIT");
    }

    println!("BEGIN_FOLLOW");
    mcp::execute(
        parse(&[
            "logs",
            "atlas",
            "--follow",
            "--logs-dir",
            logs.to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect("successful tail follow renders an output artifact");
    println!("END_FOLLOW");

    let invoked = std::fs::read_to_string(&capture).expect("tail invocation capture");
    assert_eq!(invoked, format!("-f\n{}\n", log.display()));

    unsafe { std::env::set_var("TAIL_EXIT", "9") };
    let error = mcp::execute(
        parse(&[
            "logs",
            "atlas",
            "--follow",
            "--logs-dir",
            logs.to_str().unwrap(),
        ]),
        &context(),
    )
    .await
    .expect_err("a failed tail process must be reported");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("Failed to get MCP server logs"),
        "{rendered}"
    );
    assert!(
        rendered.contains("tail -f exited with non-zero status"),
        "{rendered}"
    );
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("create follow stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("create follow stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn follow helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("poll follow helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap follow helper");
            let output = Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
            panic!(
                "follow helper timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn mcp_follow_uses_the_resolved_server_log_and_surfaces_tail_failure() {
    let mut command = Command::new(std::env::current_exe().expect("unit test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "follow helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("follow output UTF-8");
    let json = stdout
        .split_once("BEGIN_FOLLOW")
        .and_then(|(_, tail)| tail.split_once("END_FOLLOW"))
        .map(|(value, _)| value.trim())
        .expect("follow output markers");
    let artifact: serde_json::Value = serde_json::from_str(json).expect("strict follow JSON");
    assert_eq!(artifact["title"], "MCP Logs", "{artifact}");
    let sections = artifact["sections"].as_array().expect("follow sections");
    let content = |heading: &str| {
        &sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
    };
    assert_eq!(content("service"), "atlas", "{artifact}");
    assert_eq!(content("source"), "disk", "{artifact}");
    assert_eq!(content("logs"), &serde_json::json!([]), "{artifact}");
}
