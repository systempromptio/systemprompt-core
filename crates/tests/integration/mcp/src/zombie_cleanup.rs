//! An external SIGKILL flags a held child dead without the caller reaping it,
//! and a stop of an already-dead recorded PID is a clean, typed no-op.

use std::process::Command;
use std::time::{Duration, Instant};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::StopOutcome;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_sleep;

fn sigkill(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

async fn exits_within_five_seconds(pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !ProcessService::is_running(pid).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    false
}

#[tokio::test]
async fn kill_minus_9_on_running_server_is_observable_via_is_running() {
    let child = spawn_sleep(60);
    let pid = child.id();
    let running_before = ProcessService::is_running(pid).await;

    sigkill(pid);
    let gone = exits_within_five_seconds(pid).await;
    drop(child);

    assert!(
        running_before,
        "freshly spawned PID {pid} must be reported running"
    );
    assert!(
        gone,
        "PID {pid} must be reported dead after SIGKILL, zombie or not"
    );
}

#[tokio::test]
async fn stop_on_an_externally_killed_pid_is_a_noop() {
    let child = spawn_sleep(60);
    let pid = child.id();
    sigkill(pid);
    let gone = exits_within_five_seconds(pid).await;

    let outcome = ProcessService::stop(pid, &ServiceName::new("zombie-probe")).await;
    drop(child);

    assert!(gone, "PID {pid} must be reported dead after SIGKILL");
    assert!(
        matches!(outcome, Ok(StopOutcome::NotRunning)),
        "{outcome:?}"
    );
}
