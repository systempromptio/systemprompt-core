//! The CLI's one Ctrl-C handler cancels the running command and reports
//! `Interrupted`, which the binary turns into exit status 130.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use systemprompt_cli::runner::interrupt::{Interrupted, run_interruptible};

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

    let outcome = tokio::time::timeout(Duration::from_secs(10), run_interruptible(command))
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
    run_interruptible(async { Ok(()) })
        .await
        .expect("ok passes through");
    let err = run_interruptible(async { Err(anyhow::anyhow!("boom")) })
        .await
        .expect_err("errors pass through");
    assert_eq!(format!("{err}"), "boom");
}
