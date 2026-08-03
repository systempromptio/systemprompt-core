//! Liveness and port probes in `services::process::monitor` against real
//! sockets and real children, including the zombie case: an exited-but-unreaped
//! child still answers `kill(pid, 0)`, and the probe must call it dead anyway.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use systemprompt_mcp::services::process::monitor::{
    get_process_info, is_process_running, is_service_healthy,
};

fn held_port() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

fn free_port() -> u16 {
    let (listener, port) = held_port();
    drop(listener);
    port
}

// Leaves the child unreaped so it lingers as a zombie.
fn exited_but_unreaped_child() -> Option<Child> {
    let child = Command::new("true").spawn().ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if systemprompt_models::subprocess::is_zombie(child.id()) {
            return Some(child);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[tokio::test]
async fn is_service_healthy_is_true_for_a_bound_port_and_false_for_a_free_one() {
    let (_listener, port) = held_port();

    assert!(
        is_service_healthy(port).await.expect("probe runs"),
        "a listening port accepts the probe connection"
    );
    assert!(
        !is_service_healthy(free_port()).await.expect("probe runs"),
        "nothing listening means unhealthy rather than an error"
    );
}

#[test]
fn is_process_running_accepts_this_process_and_rejects_a_dead_pid() {
    assert!(is_process_running(std::process::id()));
    assert!(!is_process_running(u32::MAX));
}

#[test]
fn is_process_running_rejects_a_zombie_that_still_answers_signal_zero() {
    let Some(mut child) = exited_but_unreaped_child() else {
        return;
    };
    let pid = child.id();

    let running = is_process_running(pid);
    let _ = child.wait();

    assert!(
        !running,
        "a reaped-but-unwaited child runs no code and must probe as dead"
    );
}

#[test]
fn get_process_info_reports_pid_parent_and_command_for_this_process() {
    let info = get_process_info(std::process::id())
        .expect("ps runs")
        .expect("this process exists");

    assert_eq!(info.pid, std::process::id());
    assert_ne!(info.ppid, 0, "a real process has a real parent");
    assert!(!info.command.is_empty());
}

#[test]
fn get_process_info_reports_none_for_a_pid_that_does_not_exist() {
    assert!(
        get_process_info(u32::MAX).expect("ps runs").is_none(),
        "an absent process yields None rather than an error"
    );
}

#[test]
fn get_process_info_reports_the_parent_of_a_spawned_child_as_this_process() {
    let Ok(mut child) = Command::new("sleep").arg("30").spawn() else {
        return;
    };
    let info = get_process_info(child.id());
    let _ = child.kill();
    let _ = child.wait();

    let info = info.expect("ps runs").expect("the child exists");
    assert_eq!(info.ppid, std::process::id(), "we spawned it");
    assert!(info.command.contains("sleep"));
}
