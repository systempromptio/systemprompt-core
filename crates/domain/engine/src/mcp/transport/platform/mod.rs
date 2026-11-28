//! The local-server platform contract: which file a command name starts, how
//! the child is started without a console, and how it is stopped with
//! everything it started.
//!
//! [`Platform`] is the contract and each supported system implements it once:
//! Windows (the command is resolved on the search path first, `taskkill` over
//! the process tree) and Unix (the command as given, the direct child only).
//! `Host` names the implementation for the system being built, so a system
//! without one does not compile. Conditions on the target system appear in
//! this module and nowhere else in the transports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tokio::process::Command;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Unix as Host;
#[cfg(windows)]
use windows::Windows as Host;

trait Platform {
    fn resolve_program(
        command: &str,
        env: &BTreeMap<String, String>,
        cwd: Option<&Path>,
    ) -> PathBuf;

    fn end_tree(pid: u32);
}

pub(super) fn resolve_program(
    command: &str,
    env: &BTreeMap<String, String>,
    cwd: Option<&Path>,
) -> PathBuf {
    Host::resolve_program(command, env, cwd)
}

pub(super) fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    // Why: the engine is linked into a GUI-subsystem binary on Windows, where a
    // console child started without this flag opens a window and takes the focus.
    #[cfg(windows)]
    command.creation_flags(windows::CREATE_NO_WINDOW);
    command
}

pub(super) fn end_tree(pid: Option<u32>) {
    if let Some(pid) = pid {
        Host::end_tree(pid);
    }
}
