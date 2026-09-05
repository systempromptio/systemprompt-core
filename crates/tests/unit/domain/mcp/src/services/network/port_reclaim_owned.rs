//! Reclaiming a port from one of our own MCP servers.
//!
//! The existing suite covers the refusal — a port held by a process we did not
//! spawn is left alone. The other side of that decision, actually signalling a
//! process we DO own, was never driven: it needs a live process carrying this
//! installation's subprocess markers and holding the port, which is what the
//! fixture here builds.

use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_mcp::services::network::port::{cleanup_port_processes, is_port_in_use};
use systemprompt_mcp::services::process::monitor::is_process_running;

// Orphaned rather than kept as a direct child: a killed child this process has
// not `wait`ed on lingers as a zombie and still answers `kill -0`, which would
// make a successful reclaim look like a failure.
fn spawn_owned_listener(port: u16, service_name: &str) -> Option<u32> {
    let python = format!(
        "import socket,time; s=socket.socket(); \
         s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); \
         s.bind((\"127.0.0.1\", {port})); s.listen(1); time.sleep(600)"
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("python3 -c '{python}' >/dev/null 2>&1 & echo $!"))
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
            service_name,
        )
        .output()
        .ok()?;
    let pid: u32 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if is_port_in_use(port) {
            return Some(pid);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    kill(pid);
    None
}

fn kill(pid: u32) {
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

// Why: this is what a restart depends on. An MCP server that died without
// releasing its port, or one left behind by a previous run, must be reclaimed
// or the service can never come back up on its configured port. The refusal
// path is already covered; without this, the code that actually signals is
// exercised by nothing.
#[tokio::test]
async fn a_port_held_by_one_of_our_own_servers_is_reclaimed_and_the_process_dies() {
    let service = "port_reclaim_fixture";
    let port = free_port();
    let Some(pid) = spawn_owned_listener(port, service) else {
        panic!("could not stand up a marked listener on port {port}");
    };

    let outcome = cleanup_port_processes(port, service).await;

    if outcome.is_err() {
        kill(pid);
    }
    outcome.expect("a port held by our own service must be reclaimed");

    let deadline = Instant::now() + Duration::from_secs(5);
    while is_process_running(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !is_process_running(pid),
        "the process holding the port must actually be gone, not merely signalled"
    );
    kill(pid);
}

// Why: cleanup runs on every start, and the overwhelmingly common case is a
// port nobody holds. That has to be a cheap no-op rather than an error, or
// every clean boot would report a failure.
#[tokio::test]
async fn a_port_nobody_holds_is_a_no_op_rather_than_an_error() {
    cleanup_port_processes(free_port(), "port_reclaim_fixture")
        .await
        .expect("an unheld port needs no reclaiming and must not error");
}
