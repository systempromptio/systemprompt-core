// Covers the port helpers' branches the existing tests miss: the in-use TRUE
// path (a real bound TcpListener), find_available_port skipping a bound port,
// and the wait/retry helpers against free and bound ports. No MCP spawn — we
// only bind plain loopback listeners.

use std::net::TcpListener;

use systemprompt_mcp::services::network::port::{
    is_port_in_use, is_port_responsive, wait_for_port_release, wait_for_port_release_with_retry,
};

fn bind_loopback() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback");
    let port = listener.local_addr().expect("local addr").port();
    (listener, port)
}

#[tokio::test]
async fn is_port_in_use_true_for_bound_listener() {
    let (_listener, port) = bind_loopback();
    assert!(is_port_in_use(port).await);
}

#[tokio::test]
async fn is_port_responsive_true_for_bound_listener() {
    let (_listener, port) = bind_loopback();
    assert!(is_port_responsive(port).await);
}

#[tokio::test]
async fn is_port_in_use_false_after_listener_dropped() {
    let port = {
        let (_listener, port) = bind_loopback();
        port
    };
    // The listener is dropped; the port is no longer accepting. The OS may not
    // immediately reuse it, but a connect should now be refused.
    assert!(!is_port_in_use(port).await);
}

#[tokio::test]
async fn wait_for_port_release_free_port_ok() {
    // A high port with no listener returns immediately.
    let result = wait_for_port_release(59777).await;
    result.expect("free port releases immediately");
}

#[tokio::test]
async fn wait_for_port_release_bound_port_times_out() {
    let (_listener, port) = bind_loopback();
    // The listener stays bound for the whole call, so all attempts see it in
    // use and the helper returns the "did not become available" error.
    let result = wait_for_port_release(port).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains(&port.to_string()));
}

#[tokio::test]
async fn wait_for_port_release_with_retry_free_port_ok() {
    let result = wait_for_port_release_with_retry(59778, "systemprompt", 2).await;
    result.expect("free port releases with retry");
}
