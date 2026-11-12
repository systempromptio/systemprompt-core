//! [`ForegroundChildren`]: the children a CLI command runs and waits on, and
//! the stop an interrupted command sends them.
//!
//! Every child is spawned through the [`Spawner`] (so on Linux it also dies
//! with this process), stamped with [`ChildKind::Command`] and a token unique
//! to that spawn, and recorded until it is reaped.
//! [`ForegroundChildren::stop_all`] signals each recorded pid only after
//! [`stop_owned`] reads the token back from the live process, so a pid recycled
//! after the child exited is never touched. A [`ForegroundChild`] dropped
//! before it is reaped kills and reaps its process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::io::{self, Write};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use systemprompt_identifiers::ServiceName;

use super::{ChildKind, Spawner, StopOutcome, SupervisionError, mark_child, stop_owned};

/// The registry of a CLI run's foreground children; cheap to clone, shared
/// by the command that spawns and the interrupt handler that stops.
#[derive(Debug, Clone, Default)]
pub struct ForegroundChildren {
    inner: Arc<Registry>,
}

#[derive(Debug, Default)]
struct Registry {
    spawner: OnceLock<Spawner>,
    live: Mutex<HashMap<u32, ServiceName>>,
    next: AtomicU64,
}

impl Registry {
    fn spawner(&self) -> io::Result<&Spawner> {
        if let Some(spawner) = self.spawner.get() {
            return Ok(spawner);
        }
        let started = Spawner::new()?;
        Ok(self.spawner.get_or_init(|| started))
    }

    fn forget(&self, pid: u32) {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&pid);
    }
}

/// A recorded foreground child; reaping it (or dropping it) removes the
/// record.
#[derive(Debug)]
pub struct ForegroundChild {
    child: Option<Child>,
    pid: u32,
    registry: Arc<Registry>,
}

impl ForegroundChildren {
    pub fn spawn(&self, mut cmd: Command) -> io::Result<ForegroundChild> {
        let sequence = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let token = ServiceName::new(format!("cli-{}-{sequence}", std::process::id()));
        mark_child(&mut cmd, ChildKind::Command, &token);
        let mut live = self
            .inner
            .live
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let child = self.inner.spawner()?.spawn_owned_supervised(cmd)?;
        let pid = child.id();
        live.insert(pid, token);
        drop(live);
        Ok(ForegroundChild {
            child: Some(child),
            pid,
            registry: Arc::clone(&self.inner),
        })
    }

    pub fn status(&self, cmd: Command) -> io::Result<ExitStatus> {
        self.spawn(cmd)?.wait()
    }

    pub fn output(&self, mut cmd: Command) -> io::Result<Output> {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        self.spawn(cmd)?.wait_with_output()
    }

    pub fn status_with_stdin(&self, mut cmd: Command, stdin: &[u8]) -> io::Result<ExitStatus> {
        cmd.stdin(Stdio::piped());
        let mut child = self.spawn(cmd)?;
        if let Some(mut handle) = child.child.as_mut().and_then(|c| c.stdin.take()) {
            handle.write_all(stdin)?;
        }
        child.wait()
    }

    #[must_use]
    pub fn tracked(&self) -> Vec<u32> {
        self.inner
            .live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .copied()
            .collect()
    }

    pub async fn stop_all(
        &self,
        grace: Duration,
    ) -> Vec<(u32, Result<StopOutcome, SupervisionError>)> {
        let recorded: Vec<(u32, ServiceName)> = self
            .inner
            .live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(pid, token)| (*pid, token.clone()))
            .collect();
        let mut outcomes = Vec::with_capacity(recorded.len());
        for (pid, token) in recorded {
            let outcome = stop_owned(pid, ChildKind::Command, &token, grace).await;
            outcomes.push((pid, outcome));
        }
        outcomes
    }
}

impl ForegroundChild {
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.pid
    }

    pub fn wait(mut self) -> io::Result<ExitStatus> {
        let mut child = self.take()?;
        let status = child.wait();
        self.registry.forget(self.pid);
        status
    }

    pub fn wait_with_output(mut self) -> io::Result<Output> {
        let child = self.take()?;
        let output = child.wait_with_output();
        self.registry.forget(self.pid);
        output
    }

    fn take(&mut self) -> io::Result<Child> {
        self.child
            .take()
            .ok_or_else(|| io::Error::other("foreground child already reaped"))
    }
}

impl Drop for ForegroundChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(error) = child.kill() {
                tracing::debug!(pid = self.pid, error = %error, "Foreground child not killed");
            }
            if let Err(error) = child.wait() {
                tracing::warn!(pid = self.pid, error = %error, "Foreground child not reaped");
            }
        }
        self.registry.forget(self.pid);
    }
}
