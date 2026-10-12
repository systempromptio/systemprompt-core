//! Executor tests against a `/bin/sh` fake host: batching and sequence
//! numbers, cancel, wall-clock timeout, stderr tail, `home/` removal, the
//! cleared environment and process-group kill.

use std::path::Path;
use std::time::{Duration, Instant};

use systemprompt_headless::{RunEvent, RunEventPayload, RunStatus, RunnerLevel};
use tokio_util::sync::CancellationToken;

use systemprompt_headless::RunLayout;

use crate::execute_support::{ShHost, run_in, run_script, run_with};

fn runner_lines(events: &[RunEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match &e.payload {
            RunEventPayload::Runner {
                message,
                level: RunnerLevel::Info,
            } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn events_arrive_batched_with_increasing_seq() {
    let run = run_script(
        "echo INIT s-1; i=0; while [ $i -lt 120 ]; do echo line$i; i=$((i+1)); done",
        Duration::from_secs(30),
        CancellationToken::new(),
    )
    .await;
    let events = run.sink.events();
    assert_eq!(run.report.outcome.status, RunStatus::Succeeded);
    assert_eq!(runner_lines(&events).len(), 120);
    assert!(run.sink.batches.lock().unwrap().len() >= 3);
    assert!(
        run.sink
            .batches
            .lock()
            .unwrap()
            .iter()
            .all(|b| b.len() <= 50)
    );
    let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, (0..seqs.len() as i64).collect::<Vec<_>>());
    assert_eq!(run.sink.sessions.lock().unwrap().len(), 1);
    let log = std::fs::read_to_string(run.layout.logs.join("stdout.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 121);
}

#[tokio::test]
async fn resumed_run_numbers_after_seq_after() {
    let run = run_with(
        "echo a; echo b",
        Duration::from_secs(30),
        CancellationToken::new(),
        Some(9),
    )
    .await;
    let seqs: Vec<i64> = run.sink.events().iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![10, 11]);
}

#[tokio::test]
async fn cancel_kills_promptly_and_reports_cancelled() {
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let started = Instant::now();
    let (run, ()) = tokio::join!(
        run_script("echo up; sleep 100", Duration::from_secs(60), cancel),
        async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            trigger.cancel();
        }
    );
    assert_eq!(run.report.outcome.status, RunStatus::Cancelled);
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn wall_clock_times_out() {
    let run = run_script(
        "sleep 100",
        Duration::from_millis(300),
        CancellationToken::new(),
    )
    .await;
    assert_eq!(run.report.outcome.status, RunStatus::TimedOut);
    assert!(run.report.outcome.duration_ms < 2000);
}

#[tokio::test]
async fn stderr_tail_is_captured() {
    let run = run_script(
        "i=0; while [ $i -lt 80 ]; do echo err$i >&2; i=$((i+1)); done; exit 3",
        Duration::from_secs(30),
        CancellationToken::new(),
    )
    .await;
    let outcome = &run.report.outcome;
    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.exit_code, Some(3));
    assert_eq!(run.report.stderr_tail.len(), 50);
    assert_eq!(run.report.stderr_tail.last().unwrap(), "err79");
    assert!(outcome.error.as_deref().unwrap().contains("err79"));
    let stderr_events = run
        .sink
        .events()
        .into_iter()
        .filter(|e| matches!(e.payload, RunEventPayload::Stderr { .. }))
        .count();
    assert_eq!(stderr_events, 20);
    let log = std::fs::read_to_string(run.layout.logs.join("stderr.log")).unwrap();
    assert_eq!(log.lines().count(), 80);
}

#[tokio::test]
async fn home_is_removed_and_diff_reported() {
    let run = run_script(
        "echo new > made.txt",
        Duration::from_secs(30),
        CancellationToken::new(),
    )
    .await;
    assert!(!run.layout.home.exists());
    assert_eq!(run.report.diff.added, 1);
    assert_eq!(run.report.diff.entries[0].path, "made.txt");
}

#[tokio::test]
async fn child_environment_is_allowlisted() {
    let run = run_script(
        "tr '\\0' '\\n' < /proc/self/environ > env.out",
        Duration::from_secs(30),
        CancellationToken::new(),
    )
    .await;
    let env = std::fs::read_to_string(run.layout.workspace.join("env.out")).unwrap();
    let mut allowed: Vec<&str> = systemprompt_headless::execute::INHERITED_ENV.to_vec();
    // Why: /bin/sh itself exports PWD, SHLVL and `_` into its children.
    allowed.extend(["RUN_MARK", "PWD", "SHLVL", "_", "OLDPWD"]);
    for line in env.lines().filter(|l| !l.is_empty()) {
        let key = line.split('=').next().unwrap();
        assert!(allowed.contains(&key), "unexpected env key {key}");
    }
    assert!(env.contains("RUN_MARK=1"));
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .status()
        .unwrap()
        .success()
}

fn read_pid(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap().trim().to_owned()
}

#[tokio::test]
async fn grandchild_in_group_is_killed_on_cancel() {
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let (run, ()) = tokio::join!(
        run_script(
            "sleep 100 & echo $! > grandchild.pid; echo up; wait",
            Duration::from_secs(60),
            cancel
        ),
        async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            trigger.cancel();
        }
    );
    assert_eq!(run.report.outcome.status, RunStatus::Cancelled);
    let pid = read_pid(&run.layout.workspace.join("grandchild.pid"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!alive(&pid), "grandchild {pid} survived");
}

#[tokio::test]
async fn persistent_home_paths_survive_between_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    let first = ShHost {
        script: "mkdir -p ../home/keep/me && echo s1 > ../home/keep/me/session.txt".to_owned(),
        keep: &["keep/me"],
    };
    let report = run_in(&layout, &first).await;
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
    assert!(!layout.home.exists());
    assert!(layout.host.join("home-keep/keep/me/session.txt").exists());

    let second = ShHost {
        script: "test -f ../home/keep/me/session.txt".to_owned(),
        keep: &["keep/me"],
    };
    let report = run_in(&layout, &second).await;
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
    assert!(!layout.home.exists());
    assert!(layout.host.join("home-keep/keep/me/session.txt").exists());
}
