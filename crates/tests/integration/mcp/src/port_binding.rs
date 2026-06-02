//! Port discovery returns a clean `Ok(Some)` for occupied ports and
//! `Ok(None)` for free ones — never a panic or a bare `io::Error`.

use systemprompt_mcp::services::process::ProcessService;

use crate::common::{bind_ephemeral_port, spawn_tcp_accept_loop};

#[tokio::test]
async fn find_pid_by_port_returns_none_for_a_free_port() {
    let (listener, port) = bind_ephemeral_port();
    drop(listener);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = ProcessService::find_pid_by_port(port).expect("must not error");
    assert!(
        result.is_none(),
        "free port {port} reported as held by PID {result:?}"
    );
}

#[tokio::test]
async fn find_pid_by_port_returns_current_pid_for_bound_port() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();

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

    let result = ProcessService::find_process_on_port_with_name(port, "nonexistent-mcp-server")
        .expect("must not error");

    assert!(
        result.is_none(),
        "name filter must reject mismatched processes, got {result:?}"
    );

    handle.abort();
}
