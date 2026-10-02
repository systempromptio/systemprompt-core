// `stop_owned` against real agent processes: the identity gate reads the live
// process's environment, so the process must carry an agent's spawn markers,
// and the escalation path (SIGTERM ignored, SIGKILL applied) needs a process
// that traps TERM.
//
// The processes are orphaned so the stop is observed the way a restarted
// supervisor observes a child of a previous run: not ours to reap.
//
// The orphan is this crate's own test binary re-executing an ignored helper,
// not `sleep`: macOS withholds the environment of Apple's hardened-runtime
// binaries, so a `/bin/sleep` orphan could never be identified as ours there.

use std::process::Command;
use std::time::{Duration, Instant};
use systemprompt_identifiers::{AgentName, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome, Termination};

// Spawns `script` in the background of a shell that then exits, and returns the
// orphan's pid. The background job's stdout is redirected away from the
// captured pipe: an orphan holding that pipe open keeps `output()` blocked for
// as long as it lives.
const MARKER_HELPER: &str = "services::agent_orchestration::process_stop_live::marker_helper";

#[test]
#[ignore = "re-executed as an orphaned process by the identity-gate tests"]
fn marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(600));
}

async fn spawn_orphan(service_name: &AgentName, wrap: impl FnOnce(&str) -> String) -> Option<u32> {
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
            service_name.as_str(),
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
        if subprocess::is_running(pid).await {
            return Some(pid);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

async fn spawn_marked_agent(service_name: &AgentName) -> Option<u32> {
    spawn_orphan(service_name, str::to_owned).await
}

// SIG_IGN survives exec, so the exec'd helper inherits the ignored TERM.
async fn spawn_term_deaf_agent(service_name: &AgentName) -> Option<u32> {
    spawn_orphan(service_name, |cmd| format!("( trap '' TERM; exec {cmd} )")).await
}

fn unique_service(prefix: &str) -> AgentName {
    AgentName::new(format!("{prefix}_{}", uuid::Uuid::new_v4().simple()))
}

async fn reap(pid: u32) {
    let _ = subprocess::terminate_gracefully(pid, Duration::ZERO).await;
}

async fn stop(pid: u32, agent: &AgentName) -> Result<StopOutcome, subprocess::SupervisionError> {
    subprocess::stop_owned(
        pid,
        ChildKind::Agent,
        &ServiceName::of_agent(agent),
        Duration::from_secs(1),
    )
    .await
}

#[tokio::test]
async fn stop_owned_escalates_to_sigkill_when_sigterm_is_ignored() {
    let agent = unique_service("sigesc");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_term_deaf_agent(&agent).await else {
        return;
    };

    let outcome = stop(pid, &agent).await;
    reap(pid).await;

    assert!(
        matches!(outcome, Ok(StopOutcome::Stopped(Termination::Killed))),
        "a TERM-ignoring agent is reclaimed with SIGKILL: {outcome:?}"
    );
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn stop_owned_stops_an_agent_that_honours_sigterm() {
    let agent = unique_service("sigterm");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&agent).await else {
        return;
    };

    let outcome = stop(pid, &agent).await;
    reap(pid).await;

    assert!(
        matches!(outcome, Ok(StopOutcome::Stopped(Termination::Exited))),
        "a TERM-honouring agent stops on the first signal: {outcome:?}"
    );
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn stop_owned_reports_a_pid_that_is_already_gone_as_not_running() {
    let agent = unique_service("siggone");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&agent).await else {
        return;
    };
    reap(pid).await;

    assert!(matches!(
        stop(pid, &agent).await,
        Ok(StopOutcome::NotRunning)
    ));
}

#[tokio::test]
async fn stop_owned_refuses_a_pid_that_names_a_different_agent() {
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&unique_service("sigmine")).await else {
        return;
    };

    let outcome = stop(pid, &unique_service("sigother")).await;
    let still_alive = subprocess::is_running(pid).await;
    reap(pid).await;

    assert!(matches!(outcome, Ok(StopOutcome::NotOurs)), "{outcome:?}");
    assert!(
        still_alive,
        "a pid whose environ names another agent must never be signalled"
    );
}

#[tokio::test]
async fn stop_owned_refuses_an_agent_marker_read_as_an_mcp_child() {
    let agent = unique_service("sigkind");
    // skip-ok: no spawnable child process on this host
    let Some(pid) = spawn_marked_agent(&agent).await else {
        return;
    };

    let outcome = subprocess::stop_owned(
        pid,
        ChildKind::Mcp,
        &ServiceName::of_agent(&agent),
        Duration::from_secs(1),
    )
    .await;
    let still_alive = subprocess::is_running(pid).await;
    reap(pid).await;

    assert!(matches!(outcome, Ok(StopOutcome::NotOurs)), "{outcome:?}");
    assert!(still_alive, "an agent marker never proves an MCP child");
}

#[tokio::test]
async fn stop_owned_treats_a_pid_outside_the_signalable_range_as_not_running() {
    assert!(matches!(
        stop(u32::MAX, &AgentName::new("anything")).await,
        Ok(StopOutcome::NotRunning)
    ));
}
