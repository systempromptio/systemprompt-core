//! Repeated port/liveness probes must not leak file descriptors: the listener
//! lookup shells out to `lsof`, so a leaked stdio handle would grow
//! the process's open-descriptor table linearly. The realistic failure is one
//! stray handle *per call*, which the `delta <= 32` guard catches within a few
//! dozen iterations; the loop counts below are kept well above that margin
//! while bounded so the per-call subprocess spawn cost stays inside the suite's
//! per-test timeout.

use std::fs;
use systemprompt_mcp::services::process::ProcessService;

use crate::common::{spawn_sleep, spawn_tcp_accept_loop};

const SUBPROCESS_LOOKUPS: usize = 64;

fn count_open_fds() -> usize {
    fs::read_dir("/dev/fd")
        .expect("/dev/fd lists this process's open descriptors")
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_port_lookups_do_not_leak_file_descriptors() {
    let (addr, handle) = spawn_tcp_accept_loop().await;
    let port = addr.port();

    for _ in 0..16 {
        let _ = ProcessService::port_has_listener(port).await;
    }
    let baseline = count_open_fds();

    for _ in 0..SUBPROCESS_LOOKUPS {
        let _ = ProcessService::port_has_listener(port).await;
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
    let mut child = spawn_sleep(60);
    let pid = child.id();

    for _ in 0..16 {
        assert!(ProcessService::is_running(pid).await);
    }
    let baseline = count_open_fds();

    for _ in 0..SUBPROCESS_LOOKUPS {
        assert!(ProcessService::is_running(pid).await);
    }
    let after = count_open_fds();
    let delta = after.saturating_sub(baseline);
    child.kill().expect("stop the probed sleep");
    child.wait().expect("reap the probed sleep");

    assert!(
        delta <= 32,
        "FD leak in is_running: baseline={baseline}, after={after}, delta={delta} (expected ≤ 32)"
    );
}
