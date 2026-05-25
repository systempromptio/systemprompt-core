//! Zombie / SIGKILL cleanup: a server killed with SIGKILL externally must
//! be flagged as dead on the next orchestrator pass, and a subsequent
//! `force_kill` of the now-dead PID must be a clean no-op (not an
//! "operation not permitted" or "no such process" failure).

use std::process::Command;
use std::time::Duration;
use systemprompt_mcp::services::process::{ProcessService, utils};

use crate::common::spawn_sleep;

fn sigkill(pid: u32) {
    // Shell out rather than depending on `nix` here — the test only
    // needs the OS-level effect, not the typed API.
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

#[tokio::test]
async fn kill_minus_9_on_running_server_is_observable_via_is_running() {
    let mut child = spawn_sleep(60);
    let pid = child.id();

    assert!(
        ProcessService::is_running(pid),
        "freshly spawned PID {pid} must be reported running"
    );

    // External SIGKILL — simulates `kill -9 <pid>` from an operator.
    sigkill(pid);
    // Reap so we don't leak a zombie into later assertions.
    let _ = child.wait();

    // Give the kernel a moment to update the process table.
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !ProcessService::is_running(pid),
        "PID {pid} must be reported dead after SIGKILL + wait"
    );
}

#[tokio::test]
async fn force_kill_on_already_dead_pid_is_a_noop() {
    let mut child = spawn_sleep(60);
    let pid = child.id();

    // Kill and reap.
    sigkill(pid);
    let _ = child.wait();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Cleanup path must not error when the PID is already gone — the
    // orchestrator's stale-PID handler relies on this so a slow
    // reconciliation pass doesn't blow up when the operator beat it to
    // the kill.
    ProcessService::force_kill(pid).expect("force_kill on dead PID must be a clean no-op");
}

#[tokio::test]
async fn terminate_gracefully_on_already_dead_pid_is_a_noop() {
    let mut child = spawn_sleep(60);
    let pid = child.id();

    sigkill(pid);
    let _ = child.wait();
    tokio::time::sleep(Duration::from_millis(50)).await;

    ProcessService::terminate_gracefully(pid)
        .expect("terminate_gracefully on dead PID must be a clean no-op");
}

#[tokio::test]
async fn graceful_then_force_terminates_within_grace_window() {
    let child = spawn_sleep(60);
    let pid = child.id();

    let ok = utils::terminate_gracefully(pid, 250).await;
    assert!(
        ok,
        "graceful-with-fallback must report success for a live PID"
    );

    // Reap so the test process doesn't leave a zombie behind.
    let mut child = child;
    let _ = child.wait();

    // PID should now be gone.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !ProcessService::is_running(pid),
        "PID {pid} must be dead after terminate_gracefully"
    );
}
