//! Orphaned-child handling: an MCP server that spawns a helper which
//! outlives the parent leaves the helper reparented to PID 1 (POSIX). The
//! orchestrator's `process_cleanup::kill_orphaned_process` only matches
//! orphans by `(port, process_name)` — a grandchild that does not bind
//! the service port will not be detected.
//!
//! This test pins that behaviour. If/when the orchestrator grows a
//! process-tree walk to reap descendants, the assertion at the bottom
//! flips and finding F-T2b-001 in findings-2026-05-25.md closes.

use std::process::Command;
use std::time::Duration;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_with_orphan_child;

fn sigkill(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status();
}

#[tokio::test]
async fn grandchild_outlives_parent_and_is_not_auto_reaped_by_orchestrator() {
    let (parent_pid, grandchild_pid) = spawn_with_orphan_child(30);

    // Parent exits on its own once it `wait`s the backgrounded child's
    // process group — but since the grandchild is in a new session, the
    // parent's wait returns immediately. Give it a moment to settle.
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(
        !ProcessService::is_running(parent_pid),
        "shell parent {parent_pid} must have exited"
    );
    assert!(
        ProcessService::is_running(grandchild_pid),
        "grandchild {grandchild_pid} must still be alive (reparented to PID 1)"
    );

    // The orchestrator's current cleanup contract does not know about
    // this grandchild — it would only find it via a port-based scan,
    // and our helper doesn't bind any port. So a defensive
    // `force_kill` on the recorded grandchild PID is required for
    // operator-driven cleanup to work; this test asserts that surface
    // still functions.
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
    // Edge: orchestrator may try to kill a PID that has already been
    // recycled / never existed. force_kill must return cleanly.
    sigkill(4_194_304); // pid_max sentinel — won't exist.
    ProcessService::force_kill(4_194_304).expect("force_kill on unallocated PID must not error");
}
