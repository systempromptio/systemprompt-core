//! A failed or missing MCP spawn surfaces a typed error and leaks no zombie
//! or registered PID.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use systemprompt_mcp::services::process::{monitor, utils};

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
    let child = Command::new("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("`true` must be on PATH");

    let pid = child.id();
    let mut child = child;
    let _ = child.wait();

    assert!(
        !monitor::is_process_running(pid),
        "process layer must report reaped PID {pid} as not running"
    );
}

#[tokio::test]
async fn unreaped_exited_child_is_reported_dead_despite_being_a_zombie() {
    let child = Command::new("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("`true` must spawn");

    let pid = child.id();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let reported_alive = monitor::is_process_running(pid);
    drop(child);

    assert!(
        !reported_alive,
        "zombie PID {pid} must be reported dead by is_process_running"
    );
}

#[test]
fn process_exists_returns_false_for_unallocated_pid() {
    assert!(
        !utils::process_exists(4_194_305),
        "process_exists must reject above-pid_max values"
    );
}
