//! Reclaiming a port from one of our own MCP servers.
//!
//! The existing suite covers the refusal — a port held by a process we did not
//! spawn is left alone. The other side of that decision, actually signalling a
//! process we DO own, was never driven: it needs a live process carrying this
//! installation's subprocess markers and holding the port, which is what the
//! fixture here builds.

use std::process::Command;
use std::time::{Duration, Instant};
use systemprompt_identifiers::{ServiceName, SpawnNonce};

use systemprompt_loader::subprocess::{self, SpawnStamp};
use systemprompt_mcp::services::network::port::{
    PortHolderClaim, cleanup_port_processes, is_port_in_use,
};

// Orphaned rather than kept as a direct child, as a stale server left behind by
// a previous run would be: init, not this test, reaps it.
async fn spawn_owned_listener(port: u16, service_name: &str, nonce: &SpawnNonce) -> Option<u32> {
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
        .env(
            systemprompt_models::subprocess::SPAWN_NONCE_ENV,
            nonce.as_str(),
        )
        .output()
        .ok()?;
    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if is_port_in_use(port).await {
            return Some(pid);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let killed = kill(pid);
    assert!(
        killed || !subprocess::is_running(pid).await,
        "listener {pid} never bound port {port} and could not be killed"
    );
    None
}

fn kill(pid: u32) -> bool {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

// Why: this is what a restart depends on. An MCP server that died without
// releasing its port, or one left behind by a previous run, must be reclaimed
// or the service can never come back up on its configured port. The refusal
// path is already covered; without this, the code that actually signals is
// exercised by nothing.
#[tokio::test]
async fn a_port_held_by_one_of_our_own_servers_is_reclaimed_and_the_process_dies() {
    let service = "port_reclaim_fixture";
    let port = systemprompt_test_fixtures::ReservedPort::loopback().release_for_child();
    let nonce = SpawnNonce::generate();
    let Some(pid) = spawn_owned_listener(port, service, &nonce).await else {
        panic!("could not stand up a marked listener on port {port}");
    };

    let outcome = cleanup_port_processes(
        port,
        &PortHolderClaim {
            service: &ServiceName::new(service),
            stamp: &SpawnStamp::Boot(nonce),
            grace: Duration::from_secs(5),
        },
    )
    .await;

    if let Err(error) = &outcome {
        let killed = kill(pid);
        panic!(
            "a port held by our own service must be reclaimed: {error:?} (fixture kill succeeded: {killed})"
        );
    }

    let gone = !subprocess::is_running(pid).await;
    if !gone {
        let killed = kill(pid);
        panic!(
            "the process holding the port must actually be gone, not merely signalled (fixture kill succeeded: {killed})"
        );
    }
}

// Why: cleanup runs on every start, and the overwhelmingly common case is a
// port nobody holds. That has to be a cheap no-op rather than an error, or
// every clean boot would report a failure.
#[tokio::test]
async fn a_port_nobody_holds_is_a_no_op_rather_than_an_error() {
    let service = ServiceName::new("port_reclaim_fixture");
    let stamp = systemprompt_loader::subprocess::SpawnStamp::Unrecorded;
    let holder = systemprompt_mcp::services::network::PortHolderClaim {
        service: &service,
        stamp: &stamp,
        grace: std::time::Duration::from_secs(5),
    };
    cleanup_port_processes(systemprompt_test_fixtures::ReservedPort::loopback().release_for_child(), &holder)
        .await
        .expect("an unheld port needs no reclaiming and must not error");
}
