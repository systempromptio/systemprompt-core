//! The CLI's one Ctrl-C handler cancels the running command and reports
//! `Interrupted`, which the binary turns into exit status 130.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use systemprompt_cli::runner::interrupt::{InterruptShield, Interrupted, run_interruptible};
use systemprompt_loader::subprocess::ForegroundChildren;

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn send_sigint_to_self() {
    let status = std::process::Command::new("kill")
        .args(["-INT", &std::process::id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ctrl_c_cancels_the_in_flight_command_and_reports_interrupted() {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = DropFlag(Arc::clone(&dropped));
    let command = async move {
        let _held = guard;
        std::future::pending::<()>().await;
        Ok(())
    };

    let signaller = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        send_sigint_to_self();
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_interruptible(
            InterruptShield::default(),
            ForegroundChildren::default(),
            command,
        ),
    )
    .await
    .expect("the interrupt must end the command");
    signaller.await.expect("signaller");

    let err = outcome.expect_err("an interrupted command is not a success");
    assert!(err.is::<Interrupted>(), "{err:#}");
    assert!(
        dropped.load(Ordering::SeqCst),
        "the in-flight work was not cancelled"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_command_that_finishes_is_returned_unchanged() {
    run_interruptible(
        InterruptShield::default(),
        ForegroundChildren::default(),
        async { Ok(()) },
    )
    .await
    .expect("ok passes through");
    let err = run_interruptible(
        InterruptShield::default(),
        ForegroundChildren::default(),
        async { Err(anyhow::anyhow!("boom")) },
    )
    .await
    .expect_err("errors pass through");
    assert_eq!(format!("{err}"), "boom");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_forced_exit_waits_for_a_held_shield() {
    let shield = InterruptShield::default();
    let guard = shield.hold();
    assert!(shield.is_held());

    let waiting = tokio::time::timeout(Duration::from_millis(200), shield.close()).await;
    assert!(
        waiting.is_err(),
        "close returned while a critical section was running"
    );

    drop(guard);
    tokio::time::timeout(Duration::from_secs(2), shield.close())
        .await
        .expect("close returns once the section ends");
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("run kill -0")
        .success()
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ctrl_c_stops_a_foreground_child_that_ignores_sigint() {
    let children = ForegroundChildren::default();
    let spawner = children.clone();
    let (pid_tx, pid_rx) = std::sync::mpsc::channel();
    let command = async move {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "trap '' INT; exec sleep 30"]);
        let child = spawner.spawn(cmd).expect("spawn child");
        pid_tx.send(child.id()).expect("send pid");
        child.wait().expect("wait child");
        Ok(())
    };

    let signaller = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        send_sigint_to_self();
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        run_interruptible(InterruptShield::default(), children.clone(), command),
    )
    .await
    .expect("the interrupt must end the command and its child");
    signaller.await.expect("signaller");

    let pid = pid_rx.recv().expect("child pid");
    let err = outcome.expect_err("an interrupted command is not a success");
    assert!(err.is::<Interrupted>(), "{err:#}");
    assert!(
        !pid_alive(pid),
        "the foreground child {pid} outlived Ctrl-C"
    );
    assert!(
        children.tracked().is_empty(),
        "a reaped child stays recorded"
    );
}

#[cfg(unix)]
#[test]
fn a_finished_foreground_child_is_no_longer_recorded() {
    let children = ForegroundChildren::default();
    let status = children
        .status(std::process::Command::new("true"))
        .expect("run true");
    assert!(status.success());
    assert!(children.tracked().is_empty());
}
