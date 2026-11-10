//! The CLI's single Ctrl-C handler.
//!
//! [`run_interruptible`] owns one listener task for the life of a command. The
//! first Ctrl-C drops the command's future, which cancels its in-flight
//! work (open transactions roll back when their connections close, scratch
//! directories are removed), restores the terminal and ends the run with
//! [`Interrupted`]. Work stuck in blocking code cannot observe that drop, so a
//! second Ctrl-C, or the grace period running out, exits the process with
//! status 130. The API server installs its own drain-aware handler and is
//! never wrapped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::Duration;

use systemprompt_logging::CliService;
use systemprompt_traits::OwnedTask;
use tokio::sync::oneshot;

pub const INTERRUPTED_EXIT_CODE: u8 = 130;

const GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Interrupted; the command was cancelled")]
pub struct Interrupted;

pub async fn run_interruptible<F>(command: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    let (interrupted, on_interrupt) = oneshot::channel::<()>();
    let _listener = OwnedTask::spawn("cli-interrupt", listen(interrupted));
    tokio::select! {
        outcome = command => outcome,
        Ok(()) = on_interrupt => {
            restore_terminal();
            CliService::warning(&Interrupted.to_string());
            Err(Interrupted.into())
        },
    }
}

async fn listen(interrupted: oneshot::Sender<()>) {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::debug!(error = %e, "Ctrl-C listener unavailable");
        return;
    }
    if interrupted.send(()).is_err() {
        return;
    }
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        () = tokio::time::sleep(GRACE) => {},
    }
    restore_terminal();
    hard_exit();
}

#[expect(
    clippy::exit,
    reason = "a command blocked outside the async runtime cannot observe cancellation; the \
              second Ctrl-C must still end the process"
)]
fn hard_exit() {
    std::process::exit(i32::from(INTERRUPTED_EXIT_CODE));
}

pub fn restore_terminal() {
    for term in [console::Term::stdout(), console::Term::stderr()] {
        if let Err(e) = term.show_cursor() {
            tracing::debug!(error = %e, "terminal cursor not restored");
        }
    }
}
