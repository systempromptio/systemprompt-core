// Decision-logic coverage for process/cleanup.rs. Every assertion drives a
// path that must NOT signal anything: the self-PID guard, the already-dead
// short-circuit, and the identity guard in terminate_gracefully_verified
// (a live PID that is not our MCP child is left untouched). No real process
// is ever signalled — we use our own PID (alive, but carries no MCP markers)
// and definitely-dead synthetic PIDs.

use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::process::cleanup::{
    force_kill, terminate_gracefully, terminate_gracefully_verified,
};

const DEAD_PID_HIGH: u32 = 4_194_305;
const DEAD_PID_HIGHER: u32 = 4_194_306;

#[test]
fn terminate_gracefully_self_pid_is_noop_ok() {
    let me = std::process::id();
    // The self-guard returns Ok(()) before any signal is issued.
    terminate_gracefully(me).expect("self-PID must be a no-op Ok");
}

#[test]
fn terminate_gracefully_dead_pid_is_ok() {
    terminate_gracefully(DEAD_PID_HIGH).expect("dead PID short-circuits to Ok");
}

#[test]
fn force_kill_self_pid_is_noop_ok() {
    let me = std::process::id();
    force_kill(me).expect("self-PID must be a no-op Ok");
}

#[test]
fn force_kill_dead_pid_is_ok() {
    force_kill(DEAD_PID_HIGHER).expect("dead PID short-circuits to Ok");
}

#[tokio::test]
async fn verified_dead_pid_returns_ok_without_signal() {
    // process_exists(dead) is false, so the function returns before the
    // identity check or any signal.
    terminate_gracefully_verified(DEAD_PID_HIGH, "any-service")
        .await
        .expect("dead PID is a no-op Ok");
}

#[tokio::test]
async fn verified_live_non_child_pid_is_skipped() {
    // Our own process is alive but carries no SYSTEMPROMPT_SUBPROCESS /
    // MCP_SERVICE_ID markers, so the identity guard fails closed and the PID
    // is left untouched (Ok). This is the core "never kill a recycled/foreign
    // PID" guarantee — and it proves we did NOT signal ourselves.
    let me = std::process::id();
    terminate_gracefully_verified(me, "not-our-service")
        .await
        .expect("live non-child PID is skipped, returning Ok");
    // Sanity: we are still running after the call.
    assert_eq!(std::process::id(), me);
}

#[tokio::test]
async fn verified_via_process_service_facade_is_skipped() {
    let me = std::process::id();
    ProcessService::terminate_gracefully_verified(me, "facade-service")
        .await
        .expect("facade routes to the same guarded path");
    assert_eq!(std::process::id(), me);
}

#[test]
fn process_service_terminate_gracefully_self_is_ok() {
    let me = std::process::id();
    ProcessService::terminate_gracefully(me).expect("facade self-PID no-op");
}

#[test]
fn process_service_force_kill_dead_is_ok() {
    ProcessService::force_kill(DEAD_PID_HIGH).expect("facade dead-PID no-op");
}

#[test]
fn process_service_is_running_self_true() {
    assert!(ProcessService::is_running(std::process::id()));
}

#[test]
fn process_service_is_running_dead_false() {
    assert!(!ProcessService::is_running(DEAD_PID_HIGH));
}
