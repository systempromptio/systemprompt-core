//! macOS: zsh, then bash, then sh; output is UTF-8; a command runs in a process
//! group of its own and is stopped through it.
//!
//! macOS has no parent-death signal that survives `execve`, so a command is
//! started on the calling thread and is tied to the engine only through the
//! group the engine signals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use tokio::process::{Child, Command};

use super::Platform;
use super::posix::{self, Group, GroupError};
use crate::tools::shell::select::{ShellKind, ShellSpec};

const SHELLS: [&str; 3] = ["zsh", "bash", "sh"];

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct MacOs;

impl Platform for MacOs {
    type Tree = Group;
    type TreeError = GroupError;

    const UNNAMED_SHELL: ShellKind = ShellKind::Posix;

    fn default_shell() -> ShellSpec {
        posix::first_shell(&SHELLS)
    }

    fn prepare(command: &mut Command, kind: ShellKind, script: &str) {
        posix::prepare(command, kind, script);
    }

    fn spawn(&self, mut command: Command) -> io::Result<Child> {
        command.spawn()
    }

    fn adopt(child: &Child) -> Result<Group, GroupError> {
        Group::of(child)
    }

    fn decode(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }
}
