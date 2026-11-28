//! What macOS and Linux share: a POSIX command line, a process group of the
//! command's own, and the signals that stop the group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use tokio::process::{Child, Command};

use super::{Tree, powershell};
use crate::tools::shell::select::{ShellKind, ShellSpec};

const LAST_RESORT: &str = "/bin/sh";

pub(super) fn first_shell(names: &[&str]) -> ShellSpec {
    let path = names
        .iter()
        .find_map(|name| which::which(name).ok())
        .unwrap_or_else(|| PathBuf::from(LAST_RESORT));
    ShellSpec {
        path,
        kind: ShellKind::Posix,
    }
}

pub(super) fn prepare(command: &mut Command, kind: ShellKind, script: &str) {
    match kind {
        ShellKind::Pwsh | ShellKind::WindowsPowerShell => powershell(command, script),
        ShellKind::Cmd => {
            command.arg("/c").arg(script);
        },
        ShellKind::GitBash | ShellKind::Posix => {
            command.arg("-c").arg(script);
        },
    }
    command.process_group(0);
}

#[derive(Debug, thiserror::Error)]
pub(super) enum GroupError {
    #[error("the command has no process id; it was already reaped")]
    Reaped,
    #[error("process id {pid} cannot lead a process group")]
    NotALeader { pid: u32 },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Group(libc::pid_t);

impl Group {
    pub(super) fn of(child: &Child) -> Result<Self, GroupError> {
        let pid = child.id().ok_or(GroupError::Reaped)?;
        libc::pid_t::try_from(pid)
            .ok()
            .filter(|leader| *leader > 1)
            .map(Self)
            .ok_or(GroupError::NotALeader { pid })
    }
}

impl Tree for Group {
    fn ask(&self) -> bool {
        signal::send(self.0, libc::SIGTERM);
        true
    }

    fn end(&self) {
        signal::send(self.0, libc::SIGKILL);
    }
}

#[expect(
    unsafe_code,
    reason = "kill(2) is the only way to signal the process group a shell command runs in"
)]
mod signal {
    pub(super) fn send(group: libc::pid_t, signal: libc::c_int) {
        // SAFETY: kill(2) with a negative pid signals the process group created for
        // this command, whose id is greater than one; no memory is shared.
        let sent = unsafe { libc::kill(-group, signal) };
        if sent != 0 {
            tracing::trace!(
                group,
                signal,
                "process group was already gone when signalled"
            );
        }
    }
}
