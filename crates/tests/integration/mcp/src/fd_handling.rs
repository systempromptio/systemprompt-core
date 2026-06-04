//! Repeated PID/port lookups must not leak file descriptors: the process layer
//! shells out to `lsof` / `ps` / `pgrep`, so a leaked stdio handle would grow
//! `/proc/self/fd` linearly. The realistic failure is one stray handle *per
//! call*, which the `delta <= 32` guard catches within a few dozen iterations;
//! the loop counts below are kept well above that margin while bounded so the
//! per-call subprocess spawn cost stays inside the suite's per-test timeout.

use std::fs;
use std::time::Duration;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_tcp_accept_loop;

const SUBPROCESS_LOOKUPS: usize = 200;

fn count_open_fds() -> usize {
    fs::read_dir("/proc/self/fd")
        .expect("/proc/self/fd must exist on Linux")
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_port_lookups_do_not_leak_file_descriptors() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();

    for _ in 0..16 {
        let _ = ProcessService::find_pid_by_port(port);
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let baseline = count_open_fds();

    for _ in 0..SUBPROCESS_LOOKUPS {
        let _ = ProcessService::find_pid_by_port(port);
    }

    let after = count_open_fds();
    let delta = after.saturating_sub(baseline);

    handle.abort();

    assert!(
        delta <= 32,
        "FD leak: baseline={baseline}, after={after}, delta={delta} (expected ≤ 32)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_is_running_checks_do_not_leak_file_descriptors() {
    let pid = std::process::id();

    for _ in 0..16 {
        assert!(ProcessService::is_running(pid));
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let baseline = count_open_fds();

    for _ in 0..SUBPROCESS_LOOKUPS {
        assert!(ProcessService::is_running(pid));
    }
    let after = count_open_fds();
    let delta = after.saturating_sub(baseline);

    assert!(
        delta <= 32,
        "FD leak in is_running: baseline={baseline}, after={after}, delta={delta} (expected ≤ 32)"
    );
}
