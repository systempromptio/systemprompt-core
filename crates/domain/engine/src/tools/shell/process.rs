//! Starting a shell command and seeing it through: output pumped from both
//! pipes by tasks the running command owns, a timeout, cancellation, and the
//! whole process tree stopped at the end.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use harness_tasks::OwnedTask;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::decode::Decoder;
use super::select::{ShellKind, ShellSpec};
use super::tree::ProcessTree;
use crate::error::ToolError;

const PS_PRELUDE: &str = "$ProgressPreference = 'SilentlyContinue'; try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}; $OutputEncoding = [System.Text.Encoding]::UTF8; ";
const DRAIN_GRACE: Duration = Duration::from_millis(750);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(crate) fn command(spec: &ShellSpec, script: &str, cwd: &Path) -> Command {
    let mut cmd = Command::new(&spec.path);
    match spec.kind {
        ShellKind::Pwsh | ShellKind::WindowsPowerShell => {
            cmd.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]);
            cmd.arg(format!("{PS_PRELUDE}{script}"));
        },
        ShellKind::Cmd => {
            // Why: cmd.exe parses its own command line, so the script is passed verbatim
            // inside /s /c "...".
            #[cfg(windows)]
            {
                cmd.raw_arg(format!("/d /s /c \"{script}\""));
            }
            #[cfg(not(windows))]
            {
                cmd.arg("/c").arg(script);
            }
        },
        ShellKind::GitBash | ShellKind::Posix => {
            cmd.arg("-c").arg(script);
        },
    }
    cmd.current_dir(cwd)
        .env("PAGER", "cat")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    #[cfg(unix)]
    cmd.process_group(0);
    cmd
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
    Exited(Option<i32>),
    TimedOut,
    Cancelled,
}

#[derive(Debug)]
pub(crate) struct Running {
    child: Child,
    tree: ProcessTree,
    output: mpsc::UnboundedReceiver<String>,
    pumps: Vec<OwnedTask<()>>,
}

pub(crate) fn start(spec: &ShellSpec, script: &str, cwd: &Path) -> Result<Running, ToolError> {
    let mut child = command(spec, script, cwd)
        .spawn()
        .map_err(|err| ToolError::io(format!("could not start {}", spec.path.display()), err))?;
    let tree = ProcessTree::attach(&child);
    let (sender, output) = mpsc::unbounded_channel();
    let mut pumps = Vec::with_capacity(2);
    if let Some(stdout) = child.stdout.take() {
        pumps.push(pump(stdout, sender.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        pumps.push(pump(stderr, sender));
    }
    tracing::debug!(shell = %spec.path.display(), pid = ?child.id(), "shell command started");
    Ok(Running {
        child,
        tree,
        output,
        pumps,
    })
}

fn pump<R>(mut stream: R, sender: mpsc::UnboundedSender<String>) -> OwnedTask<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    OwnedTask::spawn("shell output pump", async move {
        let mut decoder = Decoder::default();
        let mut buffer = vec![0u8; 8192];
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(read) => {
                    let text = decoder.push(&buffer[..read]);
                    if !text.is_empty() && sender.send(text).is_err() {
                        return;
                    }
                },
                Err(err) => {
                    tracing::debug!(error = %err, "could not read shell command output");
                    break;
                },
            }
        }
        let rest = decoder.finish();
        if !rest.is_empty() && sender.send(rest).is_err() {
            tracing::trace!("shell command output ended after its reader stopped");
        }
    })
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

impl Running {
    pub(crate) async fn drive<F>(
        mut self,
        timeout: Option<Duration>,
        cancel: &CancellationToken,
        mut sink: F,
    ) -> Ending
    where
        F: FnMut(&str) + Send,
    {
        let deadline = timeout.map(|limit| Instant::now() + limit);
        let mut reading = true;
        let mut ending: Option<Ending> = None;
        while ending.is_none() {
            tokio::select! {
                chunk = self.output.recv(), if reading => match chunk {
                    Some(text) => sink(&text),
                    None => reading = false,
                },
                status = self.child.wait() => {
                    ending = Some(Ending::Exited(status.as_ref().ok().and_then(std::process::ExitStatus::code)));
                }
                () = cancel.cancelled() => {
                    self.tree.kill(&mut self.child).await;
                    ending = Some(Ending::Cancelled);
                }
                () = sleep_until(deadline) => {
                    self.tree.kill(&mut self.child).await;
                    ending = Some(Ending::TimedOut);
                }
            }
        }
        let grace = Instant::now() + DRAIN_GRACE;
        while reading {
            match tokio::time::timeout_at(grace, self.output.recv()).await {
                Ok(Some(text)) => sink(&text),
                Ok(None) | Err(_) => reading = false,
            }
        }
        self.tree.release();
        tracing::trace!(pumps = self.pumps.len(), "shell command driven to its end");
        ending.unwrap_or(Ending::Cancelled)
    }
}
