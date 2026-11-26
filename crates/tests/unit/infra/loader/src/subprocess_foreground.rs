use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_loader::subprocess::{self, ForegroundChildren, StopOutcome};

fn await_zombie(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !subprocess::is_zombie(pid) {
        assert!(Instant::now() < deadline, "child {pid} never exited");
        std::thread::yield_now();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stopping_an_exited_foreground_child_leaves_its_reap_to_the_owner() {
    let children = ForegroundChildren::default();
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "exit 3"]);
    let child = children.spawn(cmd).expect("spawn child");
    let pid = child.id();
    await_zombie(pid);

    let outcomes = children.stop_all(Duration::from_secs(1)).await;
    assert_eq!(outcomes.len(), 1);
    assert!(
        matches!(outcomes[0].1, Ok(StopOutcome::NotRunning)),
        "{:?}",
        outcomes[0].1
    );

    let status = child.wait().expect("the owner reaps its own child");
    assert_eq!(status.code(), Some(3));
    assert!(children.tracked().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_foreground_child_killed_by_stop_all_is_reaped_by_its_waiting_owner() {
    use std::os::unix::process::ExitStatusExt;

    let children = ForegroundChildren::default();
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "trap '' TERM; exec sleep 30"]);
    let child = children.spawn(cmd).expect("spawn child");
    let owner = tokio::task::spawn_blocking(move || child.wait());

    let outcomes = children.stop_all(Duration::from_millis(200)).await;
    assert_eq!(outcomes.len(), 1);
    assert!(
        matches!(outcomes[0].1, Ok(StopOutcome::Stopped(_))),
        "{:?}",
        outcomes[0].1
    );

    let status = owner
        .await
        .expect("owner task")
        .expect("the owner reaps its own child");
    assert!(status.signal().is_some(), "{status:?}");
    assert!(children.tracked().is_empty());
}
