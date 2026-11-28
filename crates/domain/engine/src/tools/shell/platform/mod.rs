//! The shell tool's platform contract: which shell a system prefers, how a
//! command line is handed to it, how the child is started apart from the
//! engine, how output that is not UTF-8 is read, and how a command is stopped
//! with everything it started.
//!
//! [`Platform`] is the contract and each supported system implements it once:
//! Windows (`PowerShell` 7, Git Bash, Windows `PowerShell`, cmd; console code
//! pages; a Job Object), macOS (zsh, bash, sh; UTF-8; a process group) and
//! Linux (bash, sh; the locale's charset; a process group and the parent-death
//! signal). `Host` names the implementation for the system being built, so a
//! system without one does not compile. Conditions on the target system
//! appear in this module and nowhere else in the shell tool.
//!
//! Stopping is the same everywhere: the tree is asked to stop where the system
//! has such a request, given a grace period, then ended outright, so a command
//! that timed out or was aborted leaves nothing running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::{fmt, io};

use tokio::process::{Child, Command};

use super::select::{ShellKind, ShellSpec};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux::Linux as Host;
#[cfg(target_os = "macos")]
use macos::MacOs as Host;
#[cfg(windows)]
use windows::Windows as Host;

const TERM_GRACE: Duration = Duration::from_millis(500);
const REAP_GRACE: Duration = Duration::from_secs(4);
const PS_PRELUDE: &str = "$ProgressPreference = 'SilentlyContinue'; try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}; $OutputEncoding = [System.Text.Encoding]::UTF8; ";

pub(super) trait Platform: fmt::Debug + Default + Send + Sync {
    type Tree: Tree;
    type TreeError: std::error::Error;

    const UNNAMED_SHELL: ShellKind;

    fn default_shell() -> ShellSpec;

    fn prepare(command: &mut Command, kind: ShellKind, script: &str);

    fn spawn(&self, command: Command) -> io::Result<Child>;

    fn adopt(child: &Child) -> Result<Self::Tree, Self::TreeError>;

    fn decode(bytes: &[u8]) -> String;
}

pub(super) trait Tree: fmt::Debug + Send + Sync {
    fn ask(&self) -> bool;

    fn end(&self);
}

pub(super) fn default_shell() -> ShellSpec {
    Host::default_shell()
}

pub(super) const fn unnamed_shell() -> ShellKind {
    Host::UNNAMED_SHELL
}

pub(super) fn decode(bytes: &[u8]) -> String {
    Host::decode(bytes)
}

fn powershell(command: &mut Command, script: &str) {
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]);
    command.arg(format!("{PS_PRELUDE}{script}"));
}

#[derive(Debug, Default)]
pub(super) struct Launcher {
    host: Host,
}

impl Launcher {
    pub(super) fn launch(
        &self,
        spec: &ShellSpec,
        script: &str,
        cwd: &Path,
    ) -> io::Result<(Child, ProcessTree)> {
        let mut command = Command::new(&spec.path);
        // Why: the engine is linked into a GUI-subsystem binary on Windows, where a
        // console child started without this flag opens a window and takes the focus.
        #[cfg(windows)]
        command.creation_flags(windows::CREATE_NO_WINDOW);
        Host::prepare(&mut command, spec.kind, script);
        command
            .current_dir(cwd)
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = self.host.spawn(command)?;
        let tree = ProcessTree::attach(&child);
        Ok((child, tree))
    }
}

#[derive(Debug)]
pub(super) struct ProcessTree {
    tree: Option<<Host as Platform>::Tree>,
}

impl ProcessTree {
    fn attach(child: &Child) -> Self {
        let tree = match Host::adopt(child) {
            Ok(tree) => Some(tree),
            Err(error) => {
                tracing::warn!(error = %error, "shell command is not tracked as a process tree");
                None
            },
        };
        Self { tree }
    }

    pub(super) async fn kill(&self, child: &mut Child) {
        if let Some(tree) = &self.tree {
            if tree.ask()
                && tokio::time::timeout(TERM_GRACE, child.wait())
                    .await
                    .is_err()
            {
                tracing::debug!("shell command outlived its terminate grace period");
            }
            tree.end();
        }
        if let Err(err) = child.start_kill() {
            tracing::trace!(error = %err, "shell command had already exited when killed");
        }
        match tokio::time::timeout(REAP_GRACE, child.wait()).await {
            Ok(Ok(_)) => {},
            Ok(Err(err)) => tracing::debug!(error = %err, "could not reap the shell command"),
            Err(_) => tracing::debug!("shell command did not exit after it was killed"),
        }
    }

    pub(super) fn release(&mut self) {
        if let Some(tree) = self.tree.take() {
            tree.end();
        }
    }
}
