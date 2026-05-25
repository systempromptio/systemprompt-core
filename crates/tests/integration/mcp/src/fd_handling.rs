//! File-descriptor stress: 1 000 sequential PID/port lookups must not
//! leak FDs. The process layer shells out to `lsof` / `ps` / `pgrep` on
//! each call; if any of those subprocesses' stdio handles leak, the FD
//! count grows linearly and eventually trips the soft `nofile` limit.
//!
//! Read `/proc/self/fd` before and after the burst; assert the delta is
//! bounded. We allow a small slack for tokio reactor / thread pool
//! warmup but not 1 000.

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

    // Warm up: first call may allocate persistent FDs (lsof handles,
    // /proc readers). Take the baseline after warmup.
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

    // Bound: ≤ 32 FDs of slack covers any one-shot caches the process
    // layer may legitimately keep open. A linear leak would explode
    // into the hundreds.
    assert!(
        delta <= 32,
        "FD leak: baseline={baseline}, after={after}, delta={delta} (expected ≤ 32)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_thousand_is_running_checks_do_not_leak_file_descriptors() {
    // The orchestrator's monitoring loop polls is_running() per service
    // per tick — a long uptime burns through hundreds of thousands of
    // checks. Validate the kill(pid, 0) path stays allocation-free.
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
