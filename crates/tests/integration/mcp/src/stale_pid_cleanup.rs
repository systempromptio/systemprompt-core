//! A registry PID that has since died must read as not-running on
//! reconciliation, even once the kernel recycles it.

use std::time::Duration;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_sleep;

#[tokio::test]
async fn pid_of_dead_process_is_recognised_as_not_running() {
    let mut child = spawn_sleep(1);
    let pid = child.id();

    let _ = child.wait();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !ProcessService::is_running(pid),
        "reaped PID {pid} must be flagged dead"
    );
}

#[tokio::test]
async fn pid_lookup_by_port_is_consistent_across_a_kill() {
    use crate::common::spawn_tcp_accept_loop;

    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let pid_before =
        ProcessService::find_pid_by_port(port).expect("lookup must succeed for live port");
    assert!(pid_before.is_some(), "live port must yield a PID");

    handle.abort();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let pid_after = ProcessService::find_pid_by_port(port).expect("lookup must succeed");
    assert!(
        pid_after.is_none(),
        "released port must yield no PID (got {pid_after:?})"
    );
}

#[tokio::test]
async fn force_kill_followed_by_relookup_reports_clean_state() {
    let child = spawn_sleep(60);
    let pid = child.id();

    assert!(ProcessService::is_running(pid));
    ProcessService::force_kill(pid).expect("force_kill of live PID must succeed");

    let mut child = child;
    let _ = child.wait();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !ProcessService::is_running(pid),
        "PID {pid} must be reported dead after force_kill + reap"
    );
}
