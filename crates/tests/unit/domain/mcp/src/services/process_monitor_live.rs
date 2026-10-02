//! `ProcessService::is_running` against real children, including the zombie
//! case: an exited-but-unreaped child still answers `kill(pid, 0)`, and the
//! probe must call it dead anyway.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use systemprompt_mcp::services::process::ProcessService;

fn exited_but_unreaped_child() -> Option<Child> {
    let child = Command::new("true").spawn().ok()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if systemprompt_loader::subprocess::is_zombie(child.id()) {
            return Some(child);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[tokio::test]
async fn a_live_child_is_running_and_a_dead_pid_is_not() {
    let mut child = Command::new("sleep")
        .arg("30")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep");

    let running = ProcessService::is_running(child.id()).await;
    child.kill().expect("kill sleeper");
    child.wait().expect("reap sleeper");

    assert!(running);
    assert!(!ProcessService::is_running(u32::MAX).await);
}

#[tokio::test]
async fn a_zombie_that_still_answers_signal_zero_is_not_running() {
    // skip-ok: no spawnable child process on this host
    let Some(child) = exited_but_unreaped_child() else {
        return;
    };
    let pid = child.id();

    assert!(
        !ProcessService::is_running(pid).await,
        "an exited-but-unwaited child runs no code and must probe as dead"
    );
    drop(child);
}
