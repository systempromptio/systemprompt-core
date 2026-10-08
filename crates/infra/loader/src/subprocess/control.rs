//! Async process control: liveness, ownership, port holders, termination.
//!
//! Everything a supervisor needs to decide whether a recorded pid is still
//! one of ours and to stop it. Blocking work — identity reads that retry,
//! `lsof`/`netstat`, every Windows primitive — runs under `spawn_blocking`;
//! the POSIX signal and reap syscalls are non-blocking and run inline.
//!
//! A child this process spawned and that has since exited stays a zombie until
//! it is reaped, and `kill(pid, 0)` reports a zombie as alive. Liveness here
//! therefore reaps an exited child of ours first and then treats a zombie as
//! exited, so a stop never waits out its grace period on a corpse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::ServiceName;

use super::{ChildKind, SupervisionError};

#[cfg(unix)]
use super::posix as platform;
#[cfg(windows)]
use super::winnt as platform;

const EXIT_POLL: Duration = Duration::from_millis(25);
const KILL_SETTLE: Duration = Duration::from_secs(5);

/// How a termination request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    AlreadyExited,
    Exited,
    Killed,
}

/// How a stop of a recorded, marker-verified child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    NotRunning,
    NotOurs,
    Stopped(Termination),
}

pub async fn is_running(pid: u32) -> bool {
    blocking_on_windows(move || !platform::has_exited(pid))
        .await
        .unwrap_or(false)
}

pub async fn owns(pid: u32, kind: ChildKind, service: &ServiceName) -> bool {
    let service = service.clone();
    let verdict = tokio::task::spawn_blocking(move || {
        !platform::has_exited(pid)
            && super::live_environ(pid).is_some_and(|environ| kind.identifies(&environ, &service))
    })
    .await;
    match verdict {
        Ok(owned) => owned,
        Err(e) => {
            tracing::warn!(pid, error = %e, "Child identity check did not complete; treating as not ours");
            false
        },
    }
}

pub async fn pids_listening_on(port: u16) -> Result<Vec<u32>, SupervisionError> {
    if port == 0 {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(move || super::ports::listening_pids(port)).await?
}

pub async fn process_group(pid: u32) -> Option<u32> {
    blocking_on_windows(move || platform::process_group(pid))
        .await
        .ok()
        .flatten()
}

pub async fn terminate_gracefully(
    pid: u32,
    grace: Duration,
) -> Result<Termination, SupervisionError> {
    terminate(pid, grace, false).await
}

pub async fn terminate_group_gracefully(
    pid: u32,
    grace: Duration,
) -> Result<Termination, SupervisionError> {
    let leader = blocking_on_windows(move || platform::is_group_leader(pid)).await?;
    terminate(pid, grace, leader).await
}

pub async fn stop_owned(
    pid: u32,
    kind: ChildKind,
    service: &ServiceName,
    grace: Duration,
) -> Result<StopOutcome, SupervisionError> {
    if !is_running(pid).await {
        return Ok(StopOutcome::NotRunning);
    }
    if !owns(pid, kind, service).await {
        tracing::warn!(
            pid,
            service = %service,
            "Recorded pid is alive but carries no matching child marker; leaving it untouched"
        );
        return Ok(StopOutcome::NotOurs);
    }
    terminate_group_gracefully(pid, grace)
        .await
        .map(StopOutcome::Stopped)
}

async fn terminate(
    pid: u32,
    grace: Duration,
    group: bool,
) -> Result<Termination, SupervisionError> {
    if !is_running(pid).await {
        return Ok(Termination::AlreadyExited);
    }
    let delivery = blocking_on_windows(move || platform::send(pid, false, group)).await??;
    if delivery == platform::Delivery::NoSuchProcess || wait_for_exit(pid, grace).await {
        return Ok(Termination::Exited);
    }
    let delivery = blocking_on_windows(move || platform::send(pid, true, group)).await??;
    if delivery == platform::Delivery::NoSuchProcess || wait_for_exit(pid, KILL_SETTLE).await {
        return Ok(Termination::Killed);
    }
    Err(SupervisionError::Survived { pid })
}

async fn wait_for_exit(pid: u32, within: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        if !is_running(pid).await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(EXIT_POLL).await;
    }
}

#[cfg(unix)]
fn blocking_on_windows<T, F>(work: F) -> impl Future<Output = Result<T, SupervisionError>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::future::ready(Ok(work()))
}

#[cfg(windows)]
fn blocking_on_windows<T, F>(work: F) -> impl Future<Output = Result<T, SupervisionError>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    async move { Ok(tokio::task::spawn_blocking(work).await?) }
}
