//! A registry PID that has since died must read as not-running on
//! reconciliation, and a stop of it must not signal anything.

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::StopOutcome;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::{spawn_sleep, spawn_tcp_accept_loop};

#[tokio::test]
async fn pid_of_dead_process_is_recognised_as_not_running() {
    let mut child = spawn_sleep(1);
    let pid = child.id();
    child.wait().expect("reap sleep");

    assert!(
        !ProcessService::is_running(pid).await,
        "reaped PID {pid} must be flagged dead"
    );
}

#[tokio::test]
async fn listener_lookup_is_consistent_across_a_release() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();

    let before = ProcessService::port_has_listener(port).await;
    handle.abort();
    let _ = handle.await;
    let after = ProcessService::port_has_listener(port).await;

    assert!(
        before.expect("lookup must succeed for live port"),
        "live port must have a listener"
    );
    assert!(
        !after.expect("lookup must succeed"),
        "released port must have no listener"
    );
}

#[tokio::test]
async fn stopping_a_reaped_recorded_pid_reports_not_running() {
    let mut child = spawn_sleep(60);
    let pid = child.id();
    child.kill().expect("kill sleep");
    child.wait().expect("reap sleep");

    let outcome = ProcessService::stop(pid, &ServiceName::new("stale-pid")).await;

    assert!(
        matches!(outcome, Ok(StopOutcome::NotRunning)),
        "{outcome:?}"
    );
}
