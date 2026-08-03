//! `NetworkService` port preparation and release waits against a listener this
//! test owns. The listener is held until the moment the assertion needs it
//! gone, so no window exists in which another process can claim the port.

use std::net::TcpListener;

use systemprompt_mcp::services::network::NetworkService;

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn prepare_port_is_a_no_op_while_this_process_still_holds_the_port() {
    let (listener, port) = held_port();

    assert!(
        NetworkService::is_port_responsive(port),
        "the port this test holds must probe as in use"
    );

    // The reclaim sweep skips this process's own pid, so the listener survives
    // the call — killing the caller is never the intent.
    let result = NetworkService::new().prepare_port(port).await;
    let still_held = NetworkService::is_port_responsive(port);
    drop(listener);

    result.expect("preparing an occupied port reports no error");
    assert!(
        still_held,
        "the sweep must never signal the process that called it"
    );
}

#[tokio::test]
async fn is_port_responsive_flips_to_false_once_the_listener_is_dropped() {
    let (listener, port) = held_port();
    assert!(NetworkService::is_port_responsive(port));

    drop(listener);

    assert!(
        !NetworkService::is_port_responsive(port),
        "a closed listener refuses the probe"
    );
}

#[tokio::test]
async fn wait_for_port_release_returns_once_the_listener_is_gone() {
    let (listener, port) = held_port();
    drop(listener);

    NetworkService::new()
        .wait_for_port_release(port)
        .await
        .expect("a released port needs no waiting");
}

#[tokio::test]
async fn wait_for_port_release_with_retry_gives_up_on_a_port_that_stays_bound() {
    let (listener, port) = held_port();

    let result = NetworkService::new()
        .wait_for_port_release_with_retry(port, 2)
        .await;
    drop(listener);

    let err = result.expect_err("a held port never releases");
    assert!(
        err.to_string().contains(&port.to_string()),
        "the failure names the port: {err}"
    );
}

#[tokio::test]
async fn wait_for_port_release_with_retry_succeeds_for_a_released_port() {
    let (listener, port) = held_port();
    drop(listener);

    NetworkService::new()
        .wait_for_port_release_with_retry(port, 2)
        .await
        .expect("a released port passes on the first attempt");

    NetworkService::cleanup_port_resources(port);
}
