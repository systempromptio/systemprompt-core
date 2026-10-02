//! Port discovery reports listeners cleanly for occupied and free ports, and
//! never counts an unmarked listener as one of this service's processes.

use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::{bind_ephemeral_port, spawn_tcp_accept_loop};

#[tokio::test]
async fn a_free_port_has_no_listener() {
    let (listener, port) = bind_ephemeral_port();
    drop(listener);

    let held = ProcessService::port_has_listener(port)
        .await
        .expect("must not error");

    assert!(!held, "free port {port} reported as held");
}

#[tokio::test]
async fn a_bound_port_has_a_listener() {
    let (addr, handle) = spawn_tcp_accept_loop().await;

    let held = ProcessService::port_has_listener(addr.port()).await;
    handle.abort();

    assert!(
        held.expect("must not error"),
        "a bound port must have a listener"
    );
}

#[tokio::test]
async fn an_unmarked_listener_is_not_an_owned_port_holder() {
    let (addr, handle) = spawn_tcp_accept_loop().await;

    let owned =
        ProcessService::owned_port_holders(addr.port(), &ServiceName::new("nonexistent-mcp")).await;
    handle.abort();

    assert_eq!(
        owned.expect("must not error"),
        Vec::<u32>::new(),
        "the unmarked test process is never an owned holder"
    );
}
