//! The workspace platform contract: the case rule of worktree keys, which key
//! segments name something outside their directory, the names a project-local
//! command shim goes by, how a formatter is started without a console, and
//! how a file's write and execute permissions are read and changed.
//!
//! [`Platform`] is the contract and each supported system implements it once:
//! Windows (keys compared in lower case, `.cmd` and `.exe` shims, one
//! read-only attribute and no executable bit) and Unix (keys as written, the
//! bare shim name, mode bits). `Host` names the implementation for the system
//! being built, so a system without one does not compile. Conditions on the
//! target system appear in this module and nowhere else in the workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

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
    fn fold(value: &str) -> String;

    fn leaves_directory(segment: &str) -> bool;

    fn shim_names(command: &str) -> Vec<String>;

    fn make_writable(path: &Path, permissions: &fs::Permissions) -> io::Result<()>;

    fn set_executable(path: &Path, executable: bool) -> io::Result<()>;

    fn is_executable(metadata: &fs::Metadata) -> bool;
}

pub(in crate::workspace) fn fold(value: &str) -> String {
    Host::fold(value)
}

pub(in crate::workspace) fn leaves_directory(segment: &str) -> bool {
    Host::leaves_directory(segment)
}

pub(in crate::workspace) fn shim_names(command: &str) -> Vec<String> {
    Host::shim_names(command)
}

pub(in crate::workspace) fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    // Why: the engine is linked into a GUI-subsystem binary on Windows, where a
    // console child started without this flag opens a window and takes the focus.
    #[cfg(windows)]
    command.creation_flags(windows::CREATE_NO_WINDOW);
    command
}

pub(in crate::workspace) fn make_writable(
    path: &Path,
    permissions: &fs::Permissions,
) -> io::Result<()> {
    Host::make_writable(path, permissions)
}

pub(in crate::workspace) fn set_executable(path: &Path, executable: bool) -> io::Result<()> {
    Host::set_executable(path, executable)
}

pub(in crate::workspace) fn is_executable(metadata: &fs::Metadata) -> bool {
    Host::is_executable(metadata)
}
