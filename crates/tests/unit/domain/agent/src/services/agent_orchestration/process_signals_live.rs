// Signal escalation and the identity gate, driven with real processes.
//
// The escalation path (SIGTERM ignored, SIGKILL applied) needs a process that
// traps TERM; the identity gate reads the live process's environment, so the
// process must carry this agent's spawn markers rather than the test process's
// own pid being registered under an agent name.
//
// The processes are deliberately orphaned rather than kept as direct children:
// a killed child this process has not `wait`ed on lingers as a zombie, which
// still answers `kill(pid, 0)`, so `terminate_gracefully` could never observe
// it die. Orphans are reparented to the init reaper and disappear for real.
//
// The orphan is this crate's own test binary re-executing an ignored helper,
// not `sleep`: macOS withholds the environment of Apple's hardened-runtime
// binaries, so a `/bin/sleep` orphan could never be identified as ours there.

use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_agent::services::agent_orchestration::process::{
    force_kill_process, kill_process_verified, process_exists, terminate_gracefully,
    terminate_gracefully_verified, terminate_process,
};

// Spawns `script` in the background of a shell that then exits, and returns the
// orphan's pid. The background job's stdout is redirected away from the
// captured pipe: an orphan holding that pipe open keeps `output()` blocked for
// as long as it lives.
const MARKER_HELPER: &str = "services::agent_orchestration::process_signals_live::marker_helper";

#[test]
#[ignore = "re-executed as an orphaned process by the identity-gate tests"]
fn marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(600));
}

fn spawn_orphan(service_name: &str, wrap: impl FnOnce(&str) -> String) -> Option<u32> {
    let helper = systemprompt_test_fixtures::helper(MARKER_HELPER);

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{} >/dev/null 2>&1 & echo $!",
            wrap(helper.command())
        ))
        .env(
            systemprompt_test_fixtures::HELPER_READY_ENV,
            helper.ready_path(),
        )
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::AGENT_NAME_ENV,
            service_name,
        )
        .output()
        .ok()?;

    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;

    helper.await_ready();

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if process_exists(pid) {
            return Some(pid);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn spawn_marked_agent(service_name: &str) -> Option<u32> {
    spawn_orphan(service_name, str::to_owned)
}

// SIG_IGN survives exec, so the exec'd helper inherits the ignored TERM.
fn spawn_term_deaf_agent(service_name: &str) -> Option<u32> {
    spawn_orphan(service_name, |cmd| format!("( trap '' TERM; exec {cmd} )"))
}

fn unique_service(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

fn reap(pid: u32) {
    let _ = force_kill_process(pid);
}

fn died_within(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_exists(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[tokio::test]
async fn terminate_gracefully_escalates_to_sigkill_when_sigterm_is_ignored() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_term_deaf_agent(&unique_service("sigesc")) else {
        return;
    };

    let result = terminate_gracefully(pid, 1).await;
    reap(pid);

    result.expect("a TERM-ignoring process is still reclaimable with SIGKILL");
    assert!(
        died_within(pid, Duration::from_secs(2)),
        "the escalation must actually end the process"
    );
}

#[tokio::test]
async fn terminate_gracefully_stops_a_process_that_honours_sigterm() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&unique_service("sigterm")) else {
        return;
    };

    terminate_gracefully(pid, 3)
        .await
        .expect("a TERM-honouring process stops on the first signal");
    assert!(!process_exists(pid));
}

#[tokio::test]
async fn terminate_gracefully_returns_immediately_for_a_pid_that_is_already_gone() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&unique_service("siggone")) else {
        return;
    };
    reap(pid);
    if !died_within(pid, Duration::from_secs(2)) {
        return;
    }

    terminate_gracefully(pid, 1)
        .await
        .expect("an absent process needs no signalling");
}

#[tokio::test]
async fn terminate_gracefully_verified_signals_a_process_carrying_this_agents_markers() {
    let service = unique_service("sigown");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&service) else {
        return;
    };

    let result = terminate_gracefully_verified(pid, &service, 3).await;
    reap(pid);

    result.expect("our own process terminates cleanly");
    assert!(
        died_within(pid, Duration::from_secs(2)),
        "the marked process was signalled"
    );
}

#[tokio::test]
async fn terminate_gracefully_verified_refuses_a_pid_that_names_a_different_agent() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&unique_service("sigmine")) else {
        return;
    };

    let result = terminate_gracefully_verified(pid, &unique_service("sigother"), 1).await;
    let still_alive = process_exists(pid);
    reap(pid);

    result.expect("a stale registry pid is reported as terminated, not an error");
    assert!(
        still_alive,
        "a pid whose environ names another agent must never be signalled"
    );
}

#[test]
fn kill_process_verified_leaves_a_pid_belonging_to_another_agent_running() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&unique_service("killmine")) else {
        return;
    };

    let reported = kill_process_verified(pid, &unique_service("killother"));
    let still_alive = process_exists(pid);
    reap(pid);

    assert!(
        reported,
        "a pid that is not ours counts as already-gone so the caller clears the row"
    );
    assert!(still_alive, "but it must not actually be killed");
}

#[test]
fn kill_process_verified_kills_a_process_that_still_names_this_agent() {
    let service = unique_service("killown");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&service) else {
        return;
    };

    let killed = kill_process_verified(pid, &service);

    assert!(killed, "our own process is signalled");
    assert!(died_within(pid, Duration::from_secs(2)));
}

#[test]
fn kill_process_verified_treats_a_dead_pid_as_already_gone() {
    assert!(
        kill_process_verified(u32::MAX, "anything"),
        "a pid with no process is already gone"
    );
}

#[test]
fn signalling_verbs_refuse_a_pid_outside_the_signalable_range() {
    let err = terminate_process(u32::MAX).expect_err("u32::MAX is not a signalable pid");
    assert!(
        err.to_string().contains("Refusing to signal"),
        "unexpected error: {err}"
    );

    let err = force_kill_process(u32::MAX).expect_err("u32::MAX is not a signalable pid");
    assert!(
        err.to_string().contains("Refusing to signal"),
        "unexpected error: {err}"
    );
}
