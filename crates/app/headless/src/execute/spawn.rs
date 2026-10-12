//! Spawns the host as an owned child leading its own process group.
//!
//! [`OwnedChild`] pairs the tokio child with a [`KillGuard`] that sends
//! `SIGKILL` to the whole group when dropped, so a cancelled, timed-out or
//! panicking run never leaves a grandchild (a tool the host started) behind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use tokio::process::Child;

use super::command::build_command;
use crate::error::{HeadlessError, HeadlessResult};
use crate::host::HostInvocation;

const TERM_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) struct KillGuard {
    pgid: Option<u32>,
}

impl KillGuard {
    pub(super) fn kill_group(&self) {
        signal_group(self.pgid, GroupSignal::Kill);
    }
}

impl Drop for KillGuard {
    fn drop(&mut self) {
        self.kill_group();
    }
}

#[derive(Debug)]
pub(super) struct OwnedChild {
    pub child: Child,
    pub guard: KillGuard,
}

pub(super) fn spawn_owned_supervised(invocation: &HostInvocation) -> HeadlessResult<OwnedChild> {
    let child = build_command(invocation)
        .spawn()
        .map_err(|source| HeadlessError::Spawn {
            program: invocation.program.clone(),
            source,
        })?;
    let guard = KillGuard { pgid: child.id() };
    Ok(OwnedChild { child, guard })
}

impl OwnedChild {
    pub(super) async fn terminate_group_gracefully(&mut self) -> Option<i32> {
        signal_group(self.guard.pgid, GroupSignal::Term);
        let waited = tokio::time::timeout(TERM_GRACE, self.child.wait()).await;
        self.guard.kill_group();
        match waited {
            Ok(Ok(status)) => status.code(),
            Ok(Err(error)) => {
                tracing::warn!(%error, "headless child wait failed after SIGTERM");
                None
            },
            Err(_elapsed) => {
                if let Err(error) = self.child.kill().await {
                    tracing::warn!(%error, "headless child kill failed");
                }
                None
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum GroupSignal {
    Term,
    Kill,
}

#[cfg(unix)]
fn signal_group(pgid: Option<u32>, signal: GroupSignal) {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let Some(raw) = pgid.and_then(|pid| i32::try_from(pid).ok()) else {
        return;
    };
    let signal = match signal {
        GroupSignal::Term => Signal::SIGTERM,
        GroupSignal::Kill => Signal::SIGKILL,
    };
    match killpg(Pid::from_raw(raw), signal) {
        Ok(()) | Err(Errno::ESRCH) => {},
        Err(error) => tracing::warn!(pgid = raw, %error, "headless process group signal failed"),
    }
}

#[cfg(not(unix))]
const fn signal_group(_pgid: Option<u32>, _signal: GroupSignal) {}
