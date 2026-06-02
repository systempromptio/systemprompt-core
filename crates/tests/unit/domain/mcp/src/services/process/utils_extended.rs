// Branches in process/utils.rs the existing utils.rs test misses: the
// kill_process self-PID and out-of-range (group-targeting) guards, the
// process_exists(0) rejection via signalable_pid, and the async
// terminate_gracefully self/zero/dead decision paths. Nothing live is killed:
// the guards reject self/0/out-of-range before any signal, and the dead-PID
// SIGTERM simply fails.

use systemprompt_mcp::services::process::utils::{kill_process, process_exists, terminate_gracefully};

const DEAD_PID: u32 = 4_194_305;

#[test]
fn kill_process_self_pid_refused() {
    // The self-guard returns false without signalling.
    assert!(!kill_process(std::process::id()));
}

#[test]
fn kill_process_zero_refused() {
    // raw <= 0 (0 targets the caller's own process group) is refused.
    assert!(!kill_process(0));
}

#[test]
fn kill_process_out_of_i32_range_refused() {
    // u32 values above i32::MAX cast to a negative i32, which kill(2) reads as
    // a process group; the guard must reject them rather than broadcast.
    assert!(!kill_process(u32::MAX));
    assert!(!kill_process((i32::MAX as u32) + 1));
}

#[test]
fn process_exists_zero_is_false() {
    // signalable_pid(0) is None, so process_exists short-circuits to false.
    assert!(!process_exists(0));
}

#[test]
fn process_exists_out_of_range_is_false() {
    assert!(!process_exists(u32::MAX));
}

#[tokio::test]
async fn terminate_gracefully_self_refused() {
    // Self-PID is rejected before any signal -> false, and we survive.
    let me = std::process::id();
    assert!(!terminate_gracefully(me, 1).await);
    assert_eq!(std::process::id(), me);
}

#[tokio::test]
async fn terminate_gracefully_zero_refused() {
    assert!(!terminate_gracefully(0, 1).await);
}

#[tokio::test]
async fn terminate_gracefully_out_of_range_refused() {
    assert!(!terminate_gracefully(u32::MAX, 1).await);
}

#[tokio::test]
async fn terminate_gracefully_dead_pid_returns_false() {
    // SIGTERM to a non-existent PID fails (ESRCH) -> early false.
    assert!(!terminate_gracefully(DEAD_PID, 1).await);
}
