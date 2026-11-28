use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, OutputLimits, ShellConfig, ShellKind, ShellSpec, ShellTool, Tool, ToolError,
    ToolOutput,
};

use crate::tools::support::recorded;

fn sh(config: ShellConfig) -> ShellTool {
    let spec = ShellSpec {
        path: "/bin/sh".into(),
        kind: ShellKind::Posix,
    };
    ShellTool::with_spec(spec, config, OutputLimits::default(), BackgroundJobs::new())
}

async fn run(root: &Path, config: ShellConfig, command: &str) -> Result<ToolOutput, ToolError> {
    let (ctx, _) = recorded(root);
    sh(config).run(json!({"command": command}), &ctx).await
}

#[tokio::test]
async fn captures_output_and_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        ShellConfig::default(),
        "echo out; echo err 1>&2; exit 3",
    )
    .await
    .unwrap();
    assert_eq!(out.metadata["exit_code"], 3);
    assert!(out.output.contains("out") && out.output.contains("err"));
    assert!(out.output.ends_with("[Exit code: 3]"));
}

#[tokio::test]
async fn asks_shell_permission_with_command_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, gate) = recorded(dir.path());
    sh(ShellConfig::default())
        .run(
            json!({"command": "git status && echo done", "description": "Check status"}),
            &ctx,
        )
        .await
        .unwrap();
    let asked = gate.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].permission, "shell");
    assert_eq!(asked[0].patterns, ["git status", "echo done"]);
    assert_eq!(asked[0].always, ["git status *", "echo *"]);
    assert_eq!(asked[0].title, "Check status");
}

#[tokio::test]
async fn paths_outside_the_project_ask_external_directory() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    let (ctx, gate) = recorded(&root);
    sh(ShellConfig::default())
        .run(json!({"command": "ls ../"}), &ctx)
        .await
        .unwrap();
    let asked = gate.asked();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[1].permission, "shell");
}

#[tokio::test]
async fn timeout_stops_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let started = Instant::now();
    let out = sh(ShellConfig::default())
        .run(json!({"command": "sleep 30", "timeout": 1}), &ctx)
        .await
        .unwrap();
    assert_eq!(out.metadata["timed_out"], true);
    assert!(out.output.contains("timed out after 1 seconds"));
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn cancel_stops_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let trigger = ctx.cancel.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let result = sh(ShellConfig::default())
        .run(json!({"command": "sleep 30"}), &ctx)
        .await;
    assert!(matches!(result, Err(ToolError::Aborted)));
    canceller.await.unwrap();
}

#[tokio::test]
async fn capture_keeps_head_and_tail() {
    let dir = tempfile::tempdir().unwrap();
    let config = ShellConfig {
        capture_bytes: 1024,
        ..ShellConfig::default()
    };
    let command = "head -c 1000 /dev/zero | tr '\\0' a; sleep 0.3; \
                   head -c 10000 /dev/zero | tr '\\0' b; printf END";
    let out = run(dir.path(), config, command).await.unwrap();
    assert!(out.output.starts_with(&"a".repeat(1000)));
    assert!(out.output.contains("bytes of output omitted"));
    assert!(out.output.ends_with("END"));
}

#[tokio::test]
async fn capture_under_limit_is_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), ShellConfig::default(), "printf 'one\\ntwo\\n'")
        .await
        .unwrap();
    assert_eq!(out.output, "one\ntwo\n");
}

#[tokio::test]
async fn silent_commands_say_so() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), ShellConfig::default(), "true")
        .await
        .unwrap();
    assert_eq!(out.output, "(no output)");
}

#[tokio::test]
async fn joins_utf8_sequences_split_across_reads() {
    let dir = tempfile::tempdir().unwrap();
    let command = "printf 'h\\303'; sleep 0.2; printf '\\251llo\\n'";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert_eq!(out.output, "héllo\n");
}

#[tokio::test]
async fn streams_unfinished_lines() {
    let dir = tempfile::tempdir().unwrap();
    let command = "printf partial; sleep 0.2; printf ' line\\nnext'";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert_eq!(out.output, "partial line\nnext");
}

#[tokio::test]
async fn invalid_bytes_fall_back_without_loss_of_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let command = "printf 'ok\\n\\377\\376 bad\\n'";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert!(out.output.starts_with("ok\n"));
    assert!(out.output.ends_with(" bad\n"));
}

#[tokio::test]
async fn missing_workdirs_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let err = sh(ShellConfig::default())
        .run(json!({"command": "true", "workdir": "absent"}), &ctx)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidInput { message } if message.contains("not a directory"))
    );
}

#[tokio::test]
async fn commands_start_with_the_quiet_environment() {
    let dir = tempfile::tempdir().unwrap();
    let command = "echo \"$PAGER:$GIT_PAGER:$GIT_TERMINAL_PROMPT\"";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert_eq!(out.output, "cat:cat:0\n");
}

#[tokio::test]
async fn commands_cannot_read_input() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), ShellConfig::default(), "cat; echo done")
        .await
        .unwrap();
    assert_eq!(out.output, "done\n");
}

#[tokio::test]
async fn a_command_leads_a_process_group_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let command = "test \"$(ps -o pgid= -p $$ | tr -d ' ')\" = \"$$\" && echo own";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert_eq!(out.output, "own\n");
}

#[tokio::test]
async fn a_timeout_leaves_no_descendant() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let out = sh(ShellConfig::default())
        .run(
            json!({"command": "(sleep 3; touch survived) & sleep 30", "timeout": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(out.metadata["timed_out"], true);
    tokio::time::sleep(Duration::from_millis(3500)).await;
    assert!(!dir.path().join("survived").exists());
}

#[tokio::test]
async fn an_abort_leaves_no_descendant() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let trigger = ctx.cancel.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        trigger.cancel();
    });
    let result = sh(ShellConfig::default())
        .run(
            json!({"command": "(sleep 3; touch survived) & sleep 30"}),
            &ctx,
        )
        .await;
    canceller.await.unwrap();
    assert!(matches!(result, Err(ToolError::Aborted)));
    tokio::time::sleep(Duration::from_millis(3500)).await;
    assert!(!dir.path().join("survived").exists());
}

#[tokio::test]
async fn a_tree_that_ignores_the_polite_stop_is_ended_outright() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let started = Instant::now();
    let out = sh(ShellConfig::default())
        .run(
            json!({"command": "trap '' TERM; (sleep 7; touch survived) & sleep 30", "timeout": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(out.metadata["timed_out"], true);
    assert!(started.elapsed() < Duration::from_secs(6));
    tokio::time::sleep(Duration::from_millis(7000)).await;
    assert!(!dir.path().join("survived").exists());
}

#[tokio::test]
async fn a_command_that_ends_normally_takes_its_stragglers_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let command = "(sleep 2; touch survived) >/dev/null 2>&1 & echo started";
    let out = run(dir.path(), ShellConfig::default(), command)
        .await
        .unwrap();
    assert_eq!(out.output, "started\n");
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!dir.path().join("survived").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_command_outlives_the_thread_that_asked_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let runtime = tokio::runtime::Handle::current();
    let asker = std::thread::spawn(move || {
        let (ctx, _) = recorded(&root);
        let tool = sh(ShellConfig::default());
        let started = runtime
            .block_on(tool.run(
                json!({"command": "sleep 1; echo finished", "background": true}),
                &ctx,
            ))
            .unwrap();
        sender.send((tool, started)).unwrap();
    });
    let (_tool, started) = receiver.await.unwrap();
    asker.join().unwrap();
    let log = started.metadata["output_path"].as_str().unwrap().to_owned();
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_eq!(std::fs::read_to_string(log).unwrap(), "finished\n");
}
