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
    process::terminate_gracefully_verified(DEAD_PID, "svc", 1)
        .await
        .expect("already-gone pid terminates without error");
}

#[tokio::test]
async fn terminate_gracefully_dead_pid_is_ok() {
    process::terminate_gracefully(DEAD_PID, 1)
        .await
        .expect("already-gone pid terminates without error");
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

// The tests above only ever exercise the already-gone early returns. These
// spawn real children so the signal paths themselves run: a cooperative child
// exits on SIGTERM inside the poll loop, and a child that traps SIGTERM forces
// the escalation to SIGKILL. Without a live child, the escalation branch — the
// one that stops a wedged agent from surviving a restart — is never executed.

use std::process::{Child, Command, Stdio};

fn spawn(script: &str) -> Child {
    Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn helper child")
}

fn reap(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn process_exists_true_for_a_live_child() {
    let child = spawn("sleep 30");
    let pid = child.id();
    assert!(
        process::process_exists(pid),
        "a just-spawned child must be visible"
    );
    reap(child);
}

// `process_exists` is kill(pid, 0), which succeeds for a zombie: a signalled
// child that nobody has reaped still answers as alive. So terminate_gracefully
// only observes the exit if someone calls wait(), which is why these tests
// reap in the background the way a supervisor does. Without the reaper, the
// poll loop times out and reports "Failed to kill process even with SIGKILL"
// against a process that is already dead.
fn spawn_reaped(script: &str) -> u32 {
    let child = spawn(script);
    let pid = child.id();
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    pid
}

#[tokio::test]
async fn terminate_gracefully_stops_a_cooperative_child() {
    let pid = spawn_reaped("sleep 30");
    assert!(process::process_exists(pid));

    process::terminate_gracefully(pid, 5)
        .await
        .expect("a SIGTERM-respecting child should stop cleanly");

    assert!(
        !process::process_exists(pid),
        "the child must be gone once reaped"
    );
}

#[tokio::test]
async fn terminate_gracefully_escalates_to_sigkill_when_sigterm_is_ignored() {
    // `trap '' TERM` makes the shell ignore SIGTERM outright, so the poll loop
    // must time out and escalate. A 1s budget keeps the test quick.
    let pid = spawn_reaped("trap '' TERM; sleep 30");
    assert!(process::process_exists(pid));

    process::terminate_gracefully(pid, 1)
        .await
        .expect("SIGKILL escalation must succeed against a SIGTERM-ignoring child");

    assert!(
        !process::process_exists(pid),
        "SIGKILL cannot be trapped, so the child must be gone after escalation"
    );
}

#[test]
fn terminate_process_signals_a_live_child() {
    let mut child = spawn("sleep 30");
    let pid = child.id();

    process::terminate_process(pid).expect("SIGTERM to a live child should succeed");

    let status = child.wait().expect("child should be reapable");
    assert!(
        !status.success(),
        "a signalled child must not report success"
    );
}

#[test]
fn force_kill_process_signals_a_live_child() {
    let mut child = spawn("trap '' TERM; sleep 30");
    let pid = child.id();

    process::force_kill_process(pid).expect("SIGKILL to a live child should succeed");

    let status = child.wait().expect("child should be reapable");
    assert!(
        !status.success(),
        "SIGKILL cannot be trapped, so the child must die"
    );
}

#[test]
fn kill_process_returns_true_for_a_live_child() {
    let mut child = spawn("sleep 30");
    let pid = child.id();

    assert!(process::kill_process(pid));

    let _ = child.wait();
}
