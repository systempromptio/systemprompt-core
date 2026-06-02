//! 1 000 sequential PID/port lookups must not leak file descriptors: the
//! process layer shells out to `lsof` / `ps` / `pgrep`, so a leaked stdio
//! handle would grow `/proc/self/fd` linearly.

use std::fs;
use std::time::Duration;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::spawn_tcp_accept_loop;

fn count_open_fds() -> usize {
    fs::read_dir("/proc/self/fd")
        .expect("/proc/self/fd must exist on Linux")
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_thousand_port_lookups_do_not_leak_file_descriptors() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();

    for _ in 0..16 {
        let _ = ProcessService::find_pid_by_port(port);
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let baseline = count_open_fds();

    for _ in 0..1_000 {
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
async fn one_thousand_is_running_checks_do_not_leak_file_descriptors() {
    let pid = std::process::id();

    for _ in 0..16 {
        assert!(ProcessService::is_running(pid));
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let baseline = count_open_fds();

    for _ in 0..1_000 {
        assert!(ProcessService::is_running(pid));
    }
    let after = count_open_fds();
    let delta = after.saturating_sub(baseline);

    assert!(
        delta <= 32,
        "FD leak in is_running: baseline={baseline}, after={after}, delta={delta} (expected ≤ 32)"
    );
}
