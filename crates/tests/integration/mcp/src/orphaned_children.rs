//! A port-blind grandchild reparented to PID 1 carries no MCP marker, so the
//! process layer reports it as not ours and never signals it.

use std::process::Command;
use std::time::{Duration, Instant};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::StopOutcome;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_with_orphan_child;

fn sigkill(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

async fn exits_within(pid: u32, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if !ProcessService::is_running(pid).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    !ProcessService::is_running(pid).await
}

#[tokio::test]
async fn unmarked_grandchild_outlives_parent_and_is_never_signalled() {
    let (parent_pid, grandchild_pid) = spawn_with_orphan_child(5);

    assert!(
        !ProcessService::is_running(parent_pid).await,
        "shell parent {parent_pid} must have exited"
    );
    assert!(
        ProcessService::is_running(grandchild_pid).await,
        "grandchild {grandchild_pid} must still be alive (reparented to PID 1)"
    );

    let outcome = ProcessService::stop(grandchild_pid, &ServiceName::new("orphan-probe")).await;
    let survived = ProcessService::is_running(grandchild_pid).await;
    sigkill(grandchild_pid);

    assert!(
        matches!(outcome, Ok(StopOutcome::NotOurs)),
        "an unmarked pid is not ours: {outcome:?}"
    );
    assert!(survived, "an unmarked grandchild is never signalled");
    assert!(
        exits_within(grandchild_pid, Duration::from_secs(5)).await,
        "grandchild {grandchild_pid} must be gone after the test's own SIGKILL"
    );
}

#[tokio::test]
async fn stopping_an_unallocated_pid_reports_not_running() {
    let outcome = ProcessService::stop(4_194_304, &ServiceName::new("orphan-probe")).await;

    assert!(
        matches!(outcome, Ok(StopOutcome::NotRunning)),
        "{outcome:?}"
    );
}
