use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, OutputLimits, ShellConfig, ShellKind, ShellSpec, ShellTool, Tool,
};

use crate::tools::support::recorded;

fn cmd() -> ShellTool {
    let spec = ShellSpec::resolve(Path::new("cmd")).unwrap();
    assert_eq!(spec.kind, ShellKind::Cmd);
    ShellTool::with_spec(
        spec,
        ShellConfig::default(),
        OutputLimits::default(),
        BackgroundJobs::new(),
    )
}

#[tokio::test]
async fn cmd_takes_the_command_line_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let out = cmd()
        .run(json!({"command": "echo \"a b\" && exit 3"}), &ctx)
        .await
        .unwrap();
    assert!(out.output.contains("\"a b\""));
    assert_eq!(out.metadata["exit_code"], 3);
}

#[tokio::test]
async fn a_timeout_ends_the_job_with_everything_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let started = Instant::now();
    let command = "start /b cmd /c \"ping -n 4 127.0.0.1 >nul & echo late>survived\" & ping -n 30 127.0.0.1 >nul";
    let out = cmd()
        .run(json!({"command": command, "timeout": 1}), &ctx)
        .await
        .unwrap();
    assert_eq!(out.metadata["timed_out"], true);
    assert!(started.elapsed() < Duration::from_secs(10));
    tokio::time::sleep(Duration::from_millis(4500)).await;
    assert!(!dir.path().join("survived").exists());
}

#[tokio::test]
async fn output_that_is_not_utf8_is_read_in_the_console_code_page() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("legacy.txt"),
        [b'a', 0xE9, b'b', b'\r', b'\n'],
    )
    .unwrap();
    let (ctx, _) = recorded(dir.path());
    let out = cmd()
        .run(json!({"command": "type legacy.txt"}), &ctx)
        .await
        .unwrap();
    assert!(out.output.starts_with('a'));
    assert!(out.output.trim_end().ends_with('b'));
    assert!(!out.output.contains('\u{FFFD}'));
}
