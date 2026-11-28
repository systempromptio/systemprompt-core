//! The language-server platform contract: the native path shape, how a
//! normalised path is settled, which file a command name can run, how a server
//! is started without a console, and how a server is stopped with everything
//! it started.
//!
//! [`Platform`] is the contract and each supported system implements it once:
//! Windows (drive letters in upper case, `PATHEXT` names, `taskkill` over the
//! process tree) and Unix (paths as written, the executable bits, the direct
//! child only). `Host` names the implementation for the system being built,
//! so a system without one does not compile. Conditions on the target system
//! appear in this module and nowhere else in the language-server code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use tokio::process::{Child, Command};

use crate::lsp::uri::PathStyle;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::Unix as Host;
#[cfg(windows)]
use windows::Windows as Host;

trait Platform {
    const PATH_STYLE: PathStyle;

    fn settle(path: PathBuf) -> PathBuf;

    fn runnable(candidate: &Path, extensions: &[String]) -> Option<PathBuf>;

    async fn end_tree(child: &Child);

    fn abandon_tree(child: &mut Child);
}

pub(in crate::lsp) const PATH_STYLE: PathStyle = Host::PATH_STYLE;

pub(in crate::lsp) fn settle(path: PathBuf) -> PathBuf {
    Host::settle(path)
}

pub(in crate::lsp) fn runnable(candidate: &Path, extensions: &[String]) -> Option<PathBuf> {
    Host::runnable(candidate, extensions)
}

pub(in crate::lsp) fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    // Why: the engine is linked into a GUI-subsystem binary on Windows, where a
    // console child started without this flag opens a window and takes the focus.
    #[cfg(windows)]
    command.creation_flags(windows::CREATE_NO_WINDOW);
    command
}

pub(in crate::lsp) async fn end_tree(child: &Child) {
    Host::end_tree(child).await;
}

pub(in crate::lsp) fn abandon_tree(child: &mut Child) {
    Host::abandon_tree(child);
}
