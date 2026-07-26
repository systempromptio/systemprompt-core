use std::net::TcpListener;
use std::process::{Child, Command};

use systemprompt_mcp::services::process::cleanup::{
    cleanup_port_processes, force_kill, terminate_gracefully, terminate_gracefully_verified,
};

fn spawn_sleeper(envs: &[(&str, &str)]) -> Child {
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let child = cmd.spawn().expect("spawn sleep");
    if !envs.is_empty() {
        await_environ(child.id(), envs[envs.len() - 1].1);
    }
    child
}

/// `spawn` returns between `fork` and `exec`, and the new environment only
/// becomes visible in `/proc/<pid>/environ` at `exec`. Reading it before then
/// yields the parent's environment, so an identity-verified signal is correctly
/// skipped and the assertion under test races the scheduler.
fn await_environ(pid: u32, marker: &str) {
    for _ in 0..500 {
        if let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ"))
            && String::from_utf8_lossy(&environ).contains(marker)
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("child {pid} never exposed {marker} in its environ");
}

#[test]
fn terminate_gracefully_sigterms_live_child() {
    let mut child = spawn_sleeper(&[]);
    let pid = child.id();

    terminate_gracefully(pid).expect("signal ok");

    let status = child.wait().expect("child reaped");
    assert!(!status.success());
}

#[test]
fn force_kill_sigkills_live_child() {
    let mut child = spawn_sleeper(&[]);
    let pid = child.id();

    force_kill(pid).expect("kill ok");

    let status = child.wait().expect("child reaped");
    assert!(!status.success());
}

#[tokio::test]
async fn verified_termination_kills_marked_subprocess() {
    let mut child = spawn_sleeper(&[
        ("SYSTEMPROMPT_SUBPROCESS", "1"),
        ("MCP_SERVICE_ID", "cleanup-live-test"),
    ]);
    let pid = child.id();

    terminate_gracefully_verified(pid, "cleanup-live-test")
        .await
        .expect("verified termination ok");

    let status = child.wait().expect("child reaped");
    assert!(!status.success());
}

#[tokio::test]
async fn verified_termination_skips_child_with_wrong_service_marker() {
    let mut child = spawn_sleeper(&[
        ("SYSTEMPROMPT_SUBPROCESS", "1"),
        ("MCP_SERVICE_ID", "some-other-service"),
    ]);
    let pid = child.id();

    terminate_gracefully_verified(pid, "cleanup-live-test")
        .await
        .expect("skip is ok");

    assert!(child.try_wait().expect("try_wait").is_none());
    child.kill().expect("cleanup child");
    child.wait().expect("reap child");
}

#[tokio::test]
async fn cleanup_port_processes_never_signals_the_caller() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();

    let killed = cleanup_port_processes(port).await.expect("cleanup ok");

    assert!(killed.contains(&std::process::id()));
    assert!(listener.local_addr().is_ok());
}

#[tokio::test]
async fn cleanup_port_processes_unused_port_returns_empty() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    let killed = cleanup_port_processes(port).await.expect("cleanup ok");
    assert!(killed.is_empty());
}
