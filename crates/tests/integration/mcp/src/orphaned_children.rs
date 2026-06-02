//! A port-blind grandchild reparented to PID 1 is not auto-reaped by
//! `kill_orphaned_process`; a direct `force_kill` of the recorded PID is.

use std::process::Command;
use std::time::Duration;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_with_orphan_child;

fn sigkill(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

#[tokio::test]
async fn grandchild_outlives_parent_and_is_not_auto_reaped_by_orchestrator() {
    let (parent_pid, grandchild_pid) = spawn_with_orphan_child(30);

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(
        !ProcessService::is_running(parent_pid),
        "shell parent {parent_pid} must have exited"
    );
    assert!(
        ProcessService::is_running(grandchild_pid),
        "grandchild {grandchild_pid} must still be alive (reparented to PID 1)"
    );

    ProcessService::force_kill(grandchild_pid)
        .expect("force_kill of a known grandchild PID must succeed");

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !ProcessService::is_running(grandchild_pid),
        "grandchild {grandchild_pid} must be dead after force_kill"
    );
}

#[tokio::test]
async fn orphan_pid_outliving_force_kill_attempt_does_not_panic() {
    sigkill(4_194_304);
    ProcessService::force_kill(4_194_304).expect("force_kill on unallocated PID must not error");
}
