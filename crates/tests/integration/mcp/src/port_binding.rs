//! Port-binding contract: the process layer's port-discovery functions
//! must return a clean `Ok(Some)` for occupied ports and `Ok(None)` for
//! free ones — not panic, not bubble up a bare `io::Error`. The
//! orchestrator's `kill_orphaned_process` flow depends on this contract
//! to decide between "skip" and "kill before restart".

use systemprompt_mcp::services::process::ProcessService;

use crate::common::{bind_ephemeral_port, spawn_tcp_accept_loop};

#[tokio::test]
async fn find_pid_by_port_returns_none_for_a_free_port() {
    // Bind to learn a port, then drop the listener — the port is now
    // guaranteed unbound (kernel may still hold TIME_WAIT, but no
    // process owns it).
    let (listener, port) = bind_ephemeral_port();
    drop(listener);
    // TIME_WAIT may keep the socket in the kernel briefly even after
    // drop; a short delay gives /proc/net/tcp time to drop the entry.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = ProcessService::find_pid_by_port(port).expect("must not error");
    // After releasing, no live process holds the port.
    assert!(
        result.is_none(),
        "free port {port} reported as held by PID {result:?}"
    );
}

#[tokio::test]
async fn find_pid_by_port_returns_current_pid_for_bound_port() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    // The listener is bound inside this test process — so the only
    // possible owner is `std::process::id()`.
    let port = addr.port();

    // Give the kernel a tick to publish the bind in /proc.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let result = ProcessService::find_pid_by_port(port).expect("must not error");
    let pid = result.expect("a bound port must have an owner");

    assert_eq!(
        pid,
        std::process::id(),
        "found PID {pid} must match the test process {self_pid}",
        self_pid = std::process::id()
    );

    handle.abort();
}

#[tokio::test]
async fn find_process_on_port_with_name_filters_by_process_name() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // The current process is unlikely to be named `nonexistent-mcp-server`.
    let result = ProcessService::find_process_on_port_with_name(port, "nonexistent-mcp-server")
        .expect("must not error");

    assert!(
        result.is_none(),
        "name filter must reject mismatched processes, got {result:?}"
    );

    handle.abort();
}
