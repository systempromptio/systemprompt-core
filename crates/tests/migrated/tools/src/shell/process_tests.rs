//! The processes a command runs as: exit codes, directory, environment,
//! output under pressure, failures to start, and stopping stubborn trees.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, OutputLimits, ShellConfig, ShellKind, ShellSpec, ShellTool, Tool, ToolError,
};

use super::posix::{plain, run, run_input};
use crate::support::{Gate, Project};

#[tokio::test]
async fn output_and_exit_codes_are_reported() {
    let project = Project::new();
    let ok = run(&project, "echo fine").await.ok();
    assert_eq!(
        (ok.output.as_str(), ok.metadata["exit_code"].as_i64()),
        ("fine\n", Some(0))
    );
    let failed = run(&project, "exit 42").await.ok();
    assert_eq!(failed.metadata["exit_code"], 42);
}

#[tokio::test]
async fn ordinary_environment_variables_pass_through() {
    let project = Project::new();
    let out = run(&project, "test -n \"$PATH\" && echo has-path")
        .await
        .ok();
    assert_eq!(out.output, "has-path\n");
}

#[tokio::test]
async fn stdout_and_stderr_arrive_in_the_order_they_were_written() {
    let project = Project::new();
    let out = run(
        &project,
        "echo 1; sleep 0.1; echo 2 1>&2; sleep 0.1; echo 3; sleep 0.1; echo 4 1>&2",
    )
    .await
    .ok();
    assert_eq!(out.output, "1\n2\n3\n4\n");
}

#[tokio::test]
async fn a_large_output_is_captured_completely_under_backpressure() {
    let project = Project::new();
    let out = run(
        &project,
        "i=0; while [ $i -lt 20000 ]; do echo \"row $i\"; i=$((i+1)); done; echo end 1>&2",
    )
    .await
    .ok();
    assert!(
        out.output.contains("row 19999\nend"),
        "{}",
        &out.output[out.output.len().saturating_sub(200)..]
    );
    assert_eq!(out.metadata["exit_code"], 0);
}

#[tokio::test]
async fn delayed_output_is_still_collected_after_the_command_writes_it() {
    let project = Project::new();
    let out = run(&project, "sleep 0.5; echo after").await.ok();
    assert_eq!(out.output, "after\n");
}

#[tokio::test]
async fn commands_cannot_read_input_and_do_not_wait_for_it() {
    let project = Project::new();
    let started = Instant::now();
    let out = run(&project, "cat; echo eof").await.ok();
    assert_eq!(out.output, "eof\n");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn an_unknown_command_fails_cleanly_with_its_status() {
    let project = Project::new();
    let out = run(&project, "definitely-not-a-command-xyz").await.ok();
    assert_eq!(out.metadata["exit_code"], 127);
    assert!(
        out.output.contains("definitely-not-a-command-xyz"),
        "{}",
        out.output
    );
}

#[tokio::test]
async fn a_shell_that_cannot_be_started_is_a_clear_failure() {
    let project = Project::new();
    let spec = ShellSpec {
        path: project.root.join("no-such-shell"),
        kind: ShellKind::Posix,
    };
    let shell = ShellTool::with_spec(
        spec,
        ShellConfig::default(),
        OutputLimits::default(),
        BackgroundJobs::new(),
    );
    let err = project
        .call(&shell, json!({"command": "true"}))
        .await
        .message();
    assert!(err.contains("could not start"), "{err}");
}

#[tokio::test]
async fn a_missing_directory_is_refused_before_anything_runs() {
    let project = Project::new();
    let err = run_input(
        &project,
        json!({"command": "true", "workdir": "nope"}),
        Gate::default(),
    )
    .await
    .err();
    assert!(matches!(err, ToolError::InvalidInput { .. }));
}

#[tokio::test]
async fn pipelines_run_and_a_broken_pipe_does_not_hang() {
    let project = Project::new();
    let sorted = run(&project, "printf 'b\\na\\n' | sort").await.ok();
    assert_eq!(sorted.output, "a\nb\n");
    let started = Instant::now();
    let first = run(&project, "yes | head -n 1").await.ok();
    assert_eq!(first.output, "y\n");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn capture_finishes_even_when_a_grandchild_keeps_the_pipes_open() {
    let project = Project::new();
    let started = Instant::now();
    let out = run(&project, "sleep 30 & echo done").await.ok();
    assert!(out.output.starts_with("done\n"));
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn a_stubborn_command_is_stopped_on_timeout_with_everything_it_started() {
    let project = Project::new();
    let marker = project.root.join("survived");
    let started = Instant::now();
    let command = format!(
        "trap '' TERM; (sleep 2; touch {}) & sleep 30",
        marker.display()
    );
    let out = run_input(
        &project,
        json!({"command": command, "timeout": 1}),
        Gate::default(),
    )
    .await
    .ok();
    assert_eq!(out.metadata["timed_out"], true);
    assert!(started.elapsed() < Duration::from_secs(10));
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!marker.exists());
}

#[tokio::test]
async fn cancelling_stops_the_command_and_what_it_started() {
    let project = Project::new();
    let marker = project.root.join("survived");
    let ctx = project.context(Arc::new(Gate::default()));
    let trigger = ctx.cancel.clone();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        trigger.cancel();
    });
    let command = format!("(sleep 2; touch {}) & sleep 30", marker.display());
    let result = plain().run(json!({"command": command}), &ctx).await;
    stopper.await.unwrap();
    assert!(matches!(result, Err(ToolError::Aborted)));
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!marker.exists());
}

#[tokio::test]
async fn the_description_is_the_title_of_the_result() {
    let project = Project::new();
    let out = run_input(
        &project,
        json!({"command": "true", "description": "Check things"}),
        Gate::default(),
    )
    .await
    .ok();
    assert_eq!(out.title, "Check things");
    assert_eq!(out.metadata["description"], "Check things");
}

#[tokio::test]
async fn a_failing_command_keeps_stderr_in_the_output() {
    let project = Project::new();
    let out = run(&project, "echo diagnostics 1>&2; exit 1").await.ok();
    assert_eq!(out.output, "diagnostics\n\n[Exit code: 1]");
}
