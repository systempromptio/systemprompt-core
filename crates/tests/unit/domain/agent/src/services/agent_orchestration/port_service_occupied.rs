//! `PortService` against ports that are genuinely occupied.
//!
//! The existing suite drives these verbs at free ports, which returns at the
//! guard before any of the interesting code. Everything that decides whether a
//! port may be reclaimed — and whether the process holding it gets killed —
//! only runs when something is actually listening, so these tests hold real
//! ports with real processes.

use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_agent::services::agent_orchestration::port_service::PortService;

fn held_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

// The marker tokens are the two `is_agent_process` looks for in the `ps` args:
// the binary name and the agent-run subcommand. Passing them as extra argv
// entries puts them in the command line without changing what the process does.
//
// The listener is orphaned rather than kept as a direct child: a killed child
// this process has not `wait`ed on lingers as a zombie and still answers
// `kill -0`, which would make the reclaim look like it had failed.
fn spawn_agent_looking_listener(port: u16) -> Option<u32> {
    let python = format!(
        "import socket,time; s=socket.socket(); \
         s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); \
         s.bind((\"127.0.0.1\", {port})); s.listen(1); time.sleep(600)"
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "python3 -c '{python}' systemprompt admin agents run >/dev/null 2>&1 & echo $!"
        ))
        .output()
        .ok()?;
    let pid: u32 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
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

fn is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|s| s.success())
}

// Why: this is the guard that keeps port cleanup from killing bystanders. The
// test runner is a non-agent process holding a port, which is exactly the case
// the branch exists for, and the refusal must carry the offending command line
// so an operator can act on it.
#[tokio::test]
async fn a_port_held_by_a_non_agent_process_is_refused_with_its_command_line() {
    let service = PortService::new();
    let (listener, port) = held_port();

    let err = service
        .cleanup_port_if_needed(port)
        .await
        .expect_err("a non-agent holder must not be reclaimed");

    let message = err.to_string();
    assert!(
        message.contains("non-agent process"),
        "the refusal must say why it refused: {message}"
    );
    assert!(
        message.contains(&std::process::id().to_string()),
        "the refusal must name the holding pid: {message}"
    );
    assert!(
        message.contains("Please stop the process manually"),
        "the operator needs the remedy, not just the diagnosis: {message}"
    );
    assert!(
        !message.contains("(unknown)"),
        "the holder's command line was resolvable and must be reported: {message}"
    );
    drop(listener);
}

// Why: cleanup_agent_ports must propagate the refusal rather than counting the
// port as cleaned. Reporting success here would let a start proceed against a
// port still owned by someone else.
#[tokio::test]
async fn a_batch_cleanup_aborts_on_the_first_port_it_may_not_reclaim() {
    let service = PortService::new();
    let (listener, port) = held_port();

    let err = service
        .cleanup_agent_ports(&[port])
        .await
        .expect_err("the batch must surface the refusal");

    assert!(
        err.to_string().contains("non-agent process"),
        "the underlying reason must not be flattened: {err}"
    );
    drop(listener);
}

// Why: the converse of the guard. A port held by one of our own orphaned agent
// workers is exactly what cleanup exists to reclaim, and it must actually die.
#[tokio::test]
async fn a_port_held_by_an_orphaned_agent_process_is_reclaimed() {
    let (listener, port) = held_port();
    drop(listener);
    let Some(pid) = spawn_agent_looking_listener(port) else {
        panic!("could not stand up an agent-looking listener on port {port}");
    };

    let outcome = PortService::new().cleanup_port_if_needed(port).await;

    if outcome.is_err() {
        kill(pid);
    }
    outcome.expect("an agent-looking holder must be reclaimed");

    let deadline = Instant::now() + Duration::from_secs(5);
    while is_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        !is_alive(pid),
        "the orphaned agent process must have been killed"
    );
    kill(pid);
}

// Why: this is the preflight an agent start runs. It must name every blocked
// port with the process behind it — a bare "unavailable" leaves the operator
// with nothing to act on.
#[test]
fn verifying_ports_reports_each_blocked_port_with_its_holder() {
    let (first, first_port) = held_port();
    let (second, second_port) = held_port();

    let err = PortService::verify_all_ports_available(&[first_port, second_port])
        .expect_err("held ports must not verify as available");

    let message = err.to_string();
    assert!(message.contains("still in use"), "got {message}");
    for port in [first_port, second_port] {
        assert!(
            message.contains(&format!("Port {port}")),
            "every blocked port must be listed, {port} was not: {message}"
        );
    }
    assert_eq!(
        message.matches("PID").count(),
        2,
        "each blocked port carries its own holder: {message}"
    );
    assert!(
        !message.contains("(unknown)"),
        "both holders were resolvable: {message}"
    );

    drop(first);
    drop(second);
}
