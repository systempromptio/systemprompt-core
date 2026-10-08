//! `ProcessService::stop` against real children: a child carrying this
//! service's marker is stopped and reaped, while a child marked for another
//! service, or carrying no marker, is never signalled.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, StopOutcome};
use systemprompt_mcp::services::process::ProcessService;

const MARKER_HELPER: &str = "services::process::cleanup_live::marker_helper";

#[test]
#[ignore = "re-executed as a child process by the verified-stop tests"]
fn marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(30));
}

fn spawn_sleeper() -> Child {
    Command::new("sleep")
        .arg("30")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep")
}

#[tokio::test]
async fn stop_terminates_and_reaps_a_child_marked_for_the_service() {
    let marked = systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "cleanup-live-test");
    let pid = marked.pid();

    let outcome = ProcessService::stop(pid, &ServiceName::new("cleanup-live-test"))
        .await
        .expect("verified stop");

    assert!(matches!(outcome, StopOutcome::Stopped(_)), "{outcome:?}");
    assert!(!subprocess::is_running(pid).await);
}

#[tokio::test]
async fn stop_leaves_a_child_marked_for_another_service_running() {
    let mut marked =
        systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "some-other-service");
    let pid = marked.pid();

    let outcome = ProcessService::stop(pid, &ServiceName::new("cleanup-live-test"))
        .await
        .expect("an unowned pid is not a failure");

    assert_eq!(outcome, StopOutcome::NotOurs);
    assert!(
        marked.child.try_wait().expect("try_wait").is_none(),
        "a child whose marker names another service is left running"
    );
}

#[tokio::test]
async fn stop_leaves_an_unmarked_child_running() {
    let mut child = spawn_sleeper();
    let pid = child.id();

    let outcome = ProcessService::stop(pid, &ServiceName::new("cleanup-live-test"))
        .await
        .expect("an unowned pid is not a failure");

    assert_eq!(outcome, StopOutcome::NotOurs);
    assert!(child.try_wait().expect("try_wait").is_none());
    child.kill().expect("kill sleeper");
    child.wait().expect("reap sleeper");
}
