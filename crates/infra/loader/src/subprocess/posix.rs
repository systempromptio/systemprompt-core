//! POSIX signal and reap primitives behind [`super::control`].
//!
//! Each call here is a single non-blocking syscall (`kill`, `getpgid`,
//! `waitpid(WNOHANG)`) or a `/proc`/`sysctl` read, so the async layer may call
//! them inline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use nix::errno::Errno;
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{Pid, getpgid};

use super::SupervisionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Delivery {
    Sent,
    NoSuchProcess,
}

fn target(pid: u32) -> Result<Pid, SupervisionError> {
    match systemprompt_models::subprocess::signalable_pid(pid) {
        Some(raw) if pid != std::process::id() => Ok(Pid::from_raw(raw)),
        _ => Err(SupervisionError::NotSignalable { pid }),
    }
}

pub(super) fn exists(pid: u32) -> bool {
    target(pid).is_ok_and(|pid| signal::kill(pid, None).is_ok())
}

pub(super) fn reap_if_child(pid: u32) -> bool {
    let Ok(pid) = target(pid) else {
        return false;
    };
    matches!(
        waitpid(pid, Some(WaitPidFlag::WNOHANG)),
        Ok(WaitStatus::Exited(..) | WaitStatus::Signaled(..))
    )
}

pub(super) fn has_exited(pid: u32) -> bool {
    reap_if_child(pid) || !exists(pid) || super::is_zombie(pid)
}

pub(super) fn is_group_leader(pid: u32) -> bool {
    target(pid).is_ok_and(|leader| getpgid(Some(leader)) == Ok(leader))
}

pub(super) fn process_group(pid: u32) -> Option<u32> {
    let pid = target(pid).ok()?;
    getpgid(Some(pid))
        .ok()
        .and_then(|pgid| u32::try_from(pgid.as_raw()).ok())
}

pub(super) fn send(pid: u32, kill: bool, group: bool) -> Result<Delivery, SupervisionError> {
    let leader = target(pid)?;
    let (sig, name) = if kill {
        (Signal::SIGKILL, "SIGKILL")
    } else {
        (Signal::SIGTERM, "SIGTERM")
    };
    let addressed = if group {
        Pid::from_raw(-leader.as_raw())
    } else {
        leader
    };
    match signal::kill(addressed, sig) {
        Ok(()) => Ok(Delivery::Sent),
        Err(Errno::ESRCH) => Ok(Delivery::NoSuchProcess),
        Err(errno) => Err(SupervisionError::Signal {
            pid,
            signal: name,
            source: std::io::Error::from(errno),
        }),
    }
}
