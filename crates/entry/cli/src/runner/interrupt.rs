//! The CLI's single Ctrl-C handler.
//!
//! [`run_interruptible`] owns one listener task for the life of a command. The
//! first Ctrl-C drops the command's future, which cancels its in-flight
//! work (open transactions roll back when their connections close, scratch
//! directories are removed), restores the terminal and ends the run with
//! [`Interrupted`]. Every foreground child the command started through the
//! run's [`ForegroundChildren`] is then stopped, once its spawn token is read
//! back from the live process; this is also what releases a command blocked
//! waiting on such a child. Work stuck in other blocking code cannot observe
//! that drop, so a second Ctrl-C, or the grace period running out, exits the
//! process with status 130 — but never while an [`InterruptShield`] is held: a
//! bounded critical section (the file swap of `admin config migrate`) finishes,
//! and the exit follows it. The API server installs its own drain-aware handler
//! and is never wrapped; nor is a child host the API spawns (its descriptor's
//! `child_host`), which ends on the signal its supervisor sends rather than on
//! a two-stage interrupt that would outlive the API's own drain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_loader::subprocess::ForegroundChildren;
use systemprompt_logging::CliService;
use systemprompt_traits::OwnedTask;
use tokio::sync::oneshot;

pub const INTERRUPTED_EXIT_CODE: u8 = 130;

const GRACE: Duration = Duration::from_secs(3);
const CHILD_GRACE: Duration = Duration::from_secs(2);
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

pub async fn run_interruptible<F>(
    shield: InterruptShield,
    children: ForegroundChildren,
    command: F,
) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    let (interrupted, mut on_interrupt) = oneshot::channel::<()>();
    let _listener = OwnedTask::spawn("cli-interrupt", listen(interrupted, shield, children));
    tokio::select! {
        biased;
        Ok(()) = &mut on_interrupt => report_interrupted(),
        outcome = command => match on_interrupt.try_recv() {
            Ok(()) => report_interrupted(),
            Err(_) => outcome,
        },
    }
}

fn report_interrupted() -> anyhow::Result<()> {
    restore_terminal();
    CliService::warning(&Interrupted.to_string());
    Err(Interrupted.into())
}

async fn listen(
    interrupted: oneshot::Sender<()>,
    shield: InterruptShield,
    children: ForegroundChildren,
) {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::debug!(error = %e, "Ctrl-C listener unavailable");
        return;
    }
    if interrupted.send(()).is_err() {
        return;
    }
    stop_children(&children).await;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        () = tokio::time::sleep(GRACE) => {},
    }
    shield.close().await;
    stop_children(&children).await;
    restore_terminal();
    hard_exit();
}

async fn stop_children(children: &ForegroundChildren) {
    for (pid, outcome) in children.stop_all(CHILD_GRACE).await {
        match outcome {
            Ok(outcome) => tracing::debug!(pid, ?outcome, "Foreground child stopped"),
            Err(e) => tracing::warn!(pid, error = %e, "Foreground child could not be stopped"),
        }
    }
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
