//! A failed or missing MCP spawn surfaces a typed error and leaks no zombie
//! or registered PID.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use systemprompt_mcp::services::process::ProcessService;

#[test]
fn spawning_a_nonexistent_binary_surfaces_an_io_error_not_a_panic() {
    let bogus = PathBuf::from("/tmp/systemprompt-test-does-not-exist-xyzzy");
    let result = Command::new(&bogus)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    assert!(
        result.is_err(),
        "spawning a non-existent binary must error, not succeed"
    );
}

#[tokio::test]
async fn process_that_exits_immediately_is_recognised_as_dead() {
    let mut child = Command::new("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("`true` must be on PATH");
    let pid = child.id();
    child.wait().expect("reap true");

    assert!(
        !ProcessService::is_running(pid).await,
        "process layer must report reaped PID {pid} as not running"
    );
}

#[tokio::test]
async fn unreaped_exited_child_is_reported_dead_despite_being_a_zombie() {
    let mut child = Command::new("sh")
        .args(["-c", "exit 0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("`sh` must spawn");
    let pid = child.id();
    let mut drained = Vec::new();
    child
        .stdout
        .take()
        .expect("piped stdout")
        .read_to_end(&mut drained)
        .expect("child closed stdout on exit");

    let reported_alive = ProcessService::is_running(pid).await;
    drop(child);

    assert!(
        !reported_alive,
        "an exited, unreaped child {pid} must be reported dead"
    );
}

#[tokio::test]
async fn an_unallocated_pid_is_not_running() {
    assert!(
        !ProcessService::is_running(4_194_305).await,
        "a pid above pid_max is never running"
    );
}
