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
//! exited, so a stop never waits out its grace period on a corpse. A child
//! that still has an owner holding its `Child` (a CLI foreground child) is
//! stopped with [`stop_owned_unreaped`]: its zombie reads as exited but is
//! left for that owner to reap, so every child has exactly one reaper.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_identifiers::{ServiceName, SpawnNonce};
use systemprompt_traits::ProcessLiveness;

use super::{ChildClaim, SpawnStamp, SupervisionError};

#[cfg(unix)]
use super::posix as platform;
#[cfg(windows)]
use super::winnt as platform;

const EXIT_POLL: Duration = Duration::from_millis(25);
const KILL_SETTLE: Duration = Duration::from_secs(5);
const DISOWN_SETTLE: Duration = Duration::from_millis(500);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reaper {
    Probe,
    Owner,
}

fn has_exited(pid: u32, reaper: Reaper) -> bool {
    match reaper {
        Reaper::Probe => platform::has_exited(pid),
        Reaper::Owner => platform::has_ended(pid),
    }
}

pub async fn is_running(pid: u32) -> bool {
    running(pid, Reaper::Probe).await
}

async fn running(pid: u32, reaper: Reaper) -> bool {
    blocking_on_windows(move || !has_exited(pid, reaper))
        .await
        .unwrap_or(false)
}

pub async fn owns(pid: u32, claim: &ChildClaim) -> bool {
    owned(pid, claim, Reaper::Probe).await
}

async fn owned(pid: u32, claim: &ChildClaim, reaper: Reaper) -> bool {
    let claim = claim.clone();
    let verdict = tokio::task::spawn_blocking(move || {
        !has_exited(pid, reaper) && super::live_pid_is_subprocess(pid, &claim)
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

/// [`ProcessLiveness`] over this module's spawn markers: a pid is ours when it
/// is alive and identifies as a supervised agent or MCP child named `service`
/// carrying the boot nonce recorded with it.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubprocessLiveness;

#[async_trait]
impl ProcessLiveness for SubprocessLiveness {
    async fn owns(&self, pid: u32, service: &ServiceName, nonce: Option<&SpawnNonce>) -> bool {
        let stamp = nonce.map_or(SpawnStamp::PreNonce, |nonce| {
            SpawnStamp::Boot(nonce.clone())
        });
        let agent = ChildClaim::Agent {
            service: service.clone(),
            stamp: stamp.clone(),
        };
        let mcp = ChildClaim::Mcp {
            service: service.clone(),
            stamp,
        };
        owns(pid, &agent).await || owns(pid, &mcp).await
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
    claim: &ChildClaim,
    grace: Duration,
) -> Result<StopOutcome, SupervisionError> {
    stop(pid, claim, grace, Reaper::Probe).await
}

pub(super) async fn stop_owned_unreaped(
    pid: u32,
    claim: &ChildClaim,
    grace: Duration,
) -> Result<StopOutcome, SupervisionError> {
    stop(pid, claim, grace, Reaper::Owner).await
}

async fn stop(
    pid: u32,
    claim: &ChildClaim,
    grace: Duration,
    reaper: Reaper,
) -> Result<StopOutcome, SupervisionError> {
    if !running(pid, reaper).await {
        return Ok(StopOutcome::NotRunning);
    }
    if !owned(pid, claim, reaper).await {
        tracing::warn!(
            pid,
            service = %claim.service(),
            "Recorded pid is alive but carries no matching child marker; leaving it untouched"
        );
        return Ok(StopOutcome::NotOurs);
    }
    let leader = blocking_on_windows(move || platform::is_group_leader(pid)).await?;
    match terminate_verified(pid, grace, leader, Some(claim), reaper).await? {
        Verified::Ended(termination) => Ok(StopOutcome::Stopped(termination)),
        Verified::Disowned => {
            tracing::warn!(
                pid,
                service = %claim.service(),
                "Pid stopped carrying its child marker during the grace period; not killing it"
            );
            Ok(StopOutcome::NotOurs)
        },
    }
}

enum Verified {
    Ended(Termination),
    Disowned,
}

async fn terminate(
    pid: u32,
    grace: Duration,
    group: bool,
) -> Result<Termination, SupervisionError> {
    match terminate_verified(pid, grace, group, None, Reaper::Probe).await? {
        Verified::Ended(termination) => Ok(termination),
        Verified::Disowned => Err(SupervisionError::Survived { pid }),
    }
}

async fn terminate_verified(
    pid: u32,
    grace: Duration,
    group: bool,
    claim: Option<&ChildClaim>,
    reaper: Reaper,
) -> Result<Verified, SupervisionError> {
    if !running(pid, reaper).await {
        return Ok(Verified::Ended(Termination::AlreadyExited));
    }
    let delivery = blocking_on_windows(move || platform::send(pid, false, group)).await??;
    if delivery == platform::Delivery::NoSuchProcess || wait_for_exit(pid, grace, reaper).await {
        return Ok(Verified::Ended(Termination::Exited));
    }
    // Why: a signalled process loses its readable environment while it exits
    // (`/proc/<pid>/environ` answers EACCES once its memory is torn down), so a
    // failed identity check right after the signal is first given the exit
    // window before the pid is judged disowned.
    if let Some(claim) = claim
        && !owned(pid, claim, reaper).await
    {
        return Ok(if wait_for_exit(pid, DISOWN_SETTLE, reaper).await {
            Verified::Ended(Termination::Exited)
        } else {
            Verified::Disowned
        });
    }
    let delivery = blocking_on_windows(move || platform::send(pid, true, group)).await??;
    if delivery == platform::Delivery::NoSuchProcess
        || wait_for_exit(pid, KILL_SETTLE, reaper).await
    {
        return Ok(Verified::Ended(Termination::Killed));
    }
    Err(SupervisionError::Survived { pid })
}

async fn wait_for_exit(pid: u32, within: Duration, reaper: Reaper) -> bool {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        if !running(pid, reaper).await {
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
