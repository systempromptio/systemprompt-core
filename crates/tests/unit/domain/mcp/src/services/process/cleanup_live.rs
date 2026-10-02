use std::process::{Child, Command};
use std::time::Duration;
use systemprompt_identifiers::ServiceName;

use systemprompt_mcp::services::process::cleanup::{
    force_kill, terminate_gracefully, terminate_gracefully_verified,
};

fn spawn_sleeper() -> Child {
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    cmd.spawn().expect("spawn sleep")
}

const MARKER_HELPER: &str = "services::process::cleanup_live::marker_helper";

#[test]
#[ignore = "re-executed as a child process by the verified-termination tests"]
fn marker_helper() {
    systemprompt_test_fixtures::announce_helper_ready();
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
fn terminate_gracefully_sigterms_live_child() {
    let mut child = spawn_sleeper();
    let pid = child.id();

    terminate_gracefully(pid).expect("signal ok");

    let status = child.wait().expect("child reaped");
    assert!(!status.success());
}

#[test]
fn force_kill_sigkills_live_child() {
    let mut child = spawn_sleeper();
    let pid = child.id();

    force_kill(pid).expect("kill ok");

    let status = child.wait().expect("child reaped");
    assert!(!status.success());
}

#[tokio::test]
async fn verified_termination_kills_marked_subprocess() {
    let mut marked =
        systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "cleanup-live-test");
    let pid = marked.pid();

    terminate_gracefully_verified(pid, &ServiceName::new("cleanup-live-test"))
        .await
        .expect("verified termination ok");

    let status = marked.child.wait().expect("child reaped");
    assert!(!status.success());
}

#[tokio::test]
async fn verified_termination_skips_child_with_wrong_service_marker() {
    let mut marked =
        systemprompt_test_fixtures::spawn_marked_child(MARKER_HELPER, "some-other-service");
    let pid = marked.pid();

    terminate_gracefully_verified(pid, &ServiceName::new("cleanup-live-test"))
        .await
        .expect("skip is ok");

    assert!(
        marked.child.try_wait().expect("try_wait").is_none(),
        "a child whose marker names another service is left running"
    );
}
