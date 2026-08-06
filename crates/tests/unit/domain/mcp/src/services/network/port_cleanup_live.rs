use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use systemprompt_mcp::services::network::port::{
    cleanup_port_processes, is_port_in_use, prepare_port, wait_for_port_release_with_retry,
};

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn spawn_listener_child(port: u16) -> Child {
    let script = format!(
        "import socket,time\ns=socket.socket()\ns.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)\ns.bind(('127.0.0.1',{port}))\ns.listen()\ntime.sleep(30)"
    );
    Command::new("python3")
        .args(["-c", &script])
        .spawn()
        .expect("spawn python listener")
}

fn await_port_state(port: u16, in_use: bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while is_port_in_use(port) != in_use {
        assert!(
            Instant::now() < deadline,
            "port {port} never reached in_use={in_use}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test]
async fn cleanup_port_processes_refuses_to_kill_a_foreign_listener() {
    let port = free_port();
    let mut child = spawn_listener_child(port);
    await_port_state(port, true);

    let err = cleanup_port_processes(port, "systemprompt")
        .await
        .expect_err("a listener we did not spawn must not be reclaimed");

    assert!(err.to_string().contains(&port.to_string()));
    assert!(
        is_port_in_use(port),
        "the foreign listener must still be running"
    );

    child.kill().expect("kill test child");
    child.wait().expect("child reaped");
}

#[tokio::test]
async fn prepare_port_skips_self_held_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();

    prepare_port(port, "systemprompt")
        .await
        .expect("prepare ok");

    assert!(listener.local_addr().is_ok());
}

#[tokio::test]
async fn wait_for_port_release_with_retry_leaves_a_foreign_listener_alone() {
    let port = free_port();
    let mut child = spawn_listener_child(port);
    await_port_state(port, true);

    wait_for_port_release_with_retry(port, "systemprompt", 3)
        .await
        .expect_err("a foreign listener must not be reclaimed");

    assert!(is_port_in_use(port));

    child.kill().expect("kill test child");
    child.wait().expect("child reaped");
}

#[tokio::test]
async fn wait_for_port_release_with_retry_gives_up_on_self_held_port() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();

    let err = wait_for_port_release_with_retry(port, "systemprompt", 2)
        .await
        .unwrap_err();

    assert!(err.to_string().contains(&format!("Port {port}")));
    assert!(listener.local_addr().is_ok());
}
