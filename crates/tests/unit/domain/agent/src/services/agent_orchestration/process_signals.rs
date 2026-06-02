// Decision-logic tests for the process signal helpers.
//
// Every PID used here is non-signalable or guaranteed-dead, so no real signal
// is ever delivered:
//   - PID 0 is rejected by `signalable_pid`.
//   - PIDs above i32::MAX wrap negative under kill(2) and are rejected.
// `process_exists` therefore returns false, and the *_verified / graceful
// helpers all take their early-return (already-gone) branches.

use systemprompt_agent::services::agent_orchestration::process;

// > i32::MAX: never a live, signalable process.
const DEAD_PID: u32 = 4_000_000_000;

#[test]
fn process_exists_false_for_pid_zero() {
    assert!(!process::process_exists(0));
}

#[test]
fn process_exists_false_for_non_signalable_pid() {
    assert!(!process::process_exists(DEAD_PID));
}

#[test]
fn kill_process_verified_dead_pid_reports_gone() {
    // Dead PID => treated as already gone (true), no signal sent.
    assert!(process::kill_process_verified(DEAD_PID, "any-agent"));
}

#[test]
fn kill_process_verified_pid_zero_reports_gone() {
    assert!(process::kill_process_verified(0, "any-agent"));
}

#[tokio::test]
async fn terminate_gracefully_verified_dead_pid_is_ok() {
    // Process does not exist => returns Ok immediately, no signal sent.
    let result = process::terminate_gracefully_verified(DEAD_PID, "svc", 1).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn terminate_gracefully_dead_pid_is_ok() {
    let result = process::terminate_gracefully(DEAD_PID, 1).await;
    assert!(result.is_ok());
}

#[test]
fn terminate_process_rejects_non_signalable_pid() {
    // signalable_pid rejects this, so terminate_process errors WITHOUT signalling.
    let result = process::terminate_process(DEAD_PID);
    assert!(result.is_err());
}

#[test]
fn force_kill_process_rejects_non_signalable_pid() {
    let result = process::force_kill_process(DEAD_PID);
    assert!(result.is_err());
}

#[test]
fn terminate_process_rejects_pid_zero() {
    let result = process::terminate_process(0);
    assert!(result.is_err());
}

#[test]
fn force_kill_process_rejects_pid_zero() {
    let result = process::force_kill_process(0);
    assert!(result.is_err());
}

#[test]
fn kill_process_returns_false_for_non_signalable() {
    // terminate_process errs => kill_process returns false.
    assert!(!process::kill_process(DEAD_PID));
}

#[test]
fn is_port_in_use_false_for_likely_free_high_port() {
    // A high ephemeral port is almost certainly free; binding succeeds so this
    // reports not-in-use. (Bind-and-drop, no listener left behind.)
    assert!(!process::is_port_in_use(0));
}
