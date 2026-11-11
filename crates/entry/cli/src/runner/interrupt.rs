//! The CLI's single Ctrl-C handler.
//!
//! [`run_interruptible`] owns one listener task for the life of a command. The
//! first Ctrl-C drops the command's future, which cancels its in-flight
//! work (open transactions roll back when their connections close, scratch
//! directories are removed), restores the terminal and ends the run with
//! [`Interrupted`]. Work stuck in blocking code cannot observe that drop, so a
//! second Ctrl-C, or the grace period running out, exits the process with
//! status 130 — but never while an [`InterruptShield`] is held: a bounded
//! critical section (the file swap of `admin config migrate`) finishes, and
//! the exit follows it. The API server installs its own drain-aware handler and
//! is never wrapped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_logging::CliService;
use systemprompt_traits::OwnedTask;
use tokio::sync::oneshot;

pub const INTERRUPTED_EXIT_CODE: u8 = 130;

const GRACE: Duration = Duration::from_secs(3);
const SHIELD_POLL: Duration = Duration::from_millis(20);

/// Defers the forced exit while a critical section runs.
///
/// One shield is created per run and handed to the command through its
/// context; [`InterruptShield::hold`] returns a guard for the section.
#[derive(Debug, Clone, Default)]
pub struct InterruptShield {
    held: Arc<AtomicUsize>,
}

impl InterruptShield {
    const CLOSED: usize = usize::MAX;

    #[must_use]
    pub fn hold(&self) -> ShieldGuard {
        loop {
            let current = self.held.load(Ordering::SeqCst);
            if current == Self::CLOSED {
                std::thread::sleep(SHIELD_POLL);
                continue;
            }
            if self
                .held
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return ShieldGuard {
                    held: Arc::clone(&self.held),
                };
            }
        }
    }

    #[must_use]
    pub fn is_held(&self) -> bool {
        !matches!(self.held.load(Ordering::SeqCst), 0 | Self::CLOSED)
    }

    pub async fn close(&self) {
        while self
            .held
            .compare_exchange(0, Self::CLOSED, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tokio::time::sleep(SHIELD_POLL).await;
        }
    }
}

#[derive(Debug)]
pub struct ShieldGuard {
    held: Arc<AtomicUsize>,
}

impl Drop for ShieldGuard {
    fn drop(&mut self) {
        self.held.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Interrupted; the command was cancelled")]
pub struct Interrupted;

pub async fn run_interruptible<F>(shield: InterruptShield, command: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    let (interrupted, on_interrupt) = oneshot::channel::<()>();
    let _listener = OwnedTask::spawn("cli-interrupt", listen(interrupted, shield));
    tokio::select! {
        outcome = command => outcome,
        Ok(()) = on_interrupt => {
            restore_terminal();
            CliService::warning(&Interrupted.to_string());
            Err(Interrupted.into())
        },
    }
}

async fn listen(interrupted: oneshot::Sender<()>, shield: InterruptShield) {
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
    shield.close().await;
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
