//! Local servers: a child process speaking newline-delimited JSON-RPC on stdin
//! and stdout. Its stderr goes to tracing at debug level; the process (and on
//! Windows its whole tree) is killed when the transport goes away.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, Command};
use tokio::sync::Mutex;

use super::lines::{LineWriter, spawn_line_reader};
use super::{InboundSink, Transport};
use crate::error::McpError;

const EXIT_GRACE: Duration = Duration::from_secs(2);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy)]
pub(crate) struct LocalCommand<'a> {
    pub(crate) command: &'a str,
    pub(crate) args: &'a [String],
    pub(crate) env: &'a BTreeMap<String, String>,
    pub(crate) cwd: Option<&'a Path>,
}

pub(crate) struct StdioTransport {
    writer: LineWriter<ChildStdin>,
    child: Mutex<Option<Child>>,
    pid: Option<u32>,
    sink: InboundSink,
}

impl StdioTransport {
    pub(crate) fn spawn(
        server: &str,
        local: LocalCommand<'_>,
        sink: InboundSink,
    ) -> Result<Self, McpError> {
        let program = resolve_program(local.command, local.env, local.cwd);
        let mut command = Command::new(&program);
        command
            .args(local.args)
            .envs(local.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = local.cwd {
            command.current_dir(cwd);
        }
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        let mut child = command.spawn().map_err(|source| McpError::Spawn {
            command: local.command.to_owned(),
            source,
        })?;
        let pid = child.id();
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(McpError::Protocol("the child process has no stdio pipes"));
        };
        spawn_line_reader(stdout, sink.clone());
        sink.tasks()
            .spawn("mcp-server-stderr", log_stderr(server.to_owned(), stderr));
        tracing::debug!(server, pid = ?pid, "started local server");
        Ok(Self {
            writer: LineWriter::new(stdin),
            child: Mutex::new(Some(child)),
            pid,
            sink,
        })
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if self.child.get_mut().is_some() {
            kill_tree(self.pid);
        }
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    // JSON: one complete JSON-RPC message, written to the child's stdin as a line.
    async fn send(&self, message: Value) -> Result<(), McpError> {
        self.writer.write(&message).await
    }

    async fn close(&self) {
        self.writer.shutdown().await;
        let child = self.child.lock().await.take();
        if let Some(mut child) = child
            && tokio::time::timeout(EXIT_GRACE, child.wait())
                .await
                .is_err()
        {
            kill_tree(self.pid);
            if let Err(error) = child.kill().await {
                tracing::debug!(error = %error, "killing the local server failed");
            }
        }
        self.sink.close();
    }
}

async fn log_stderr(server: String, stderr: ChildStderr) {
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&line);
                let text = text.trim_end();
                if !text.is_empty() {
                    tracing::debug!(server = %server, line = %text, "local server stderr");
                }
            },
            Err(error) => {
                tracing::debug!(server = %server, error = %error, "reading local server stderr failed");
                break;
            },
        }
    }
}

#[cfg(windows)]
fn resolve_program(command: &str, env: &BTreeMap<String, String>, cwd: Option<&Path>) -> PathBuf {
    // Why: CreateProcess neither searches PATHEXT nor finds .cmd/.bat shims such as
    // npx, so the full path is resolved first.
    let search_path = env
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
        .map(|(_, value)| value.clone());
    let found = search_path.map_or_else(
        || which::which(command),
        |paths| which::which_in(command, Some(paths), cwd.unwrap_or_else(|| Path::new("."))),
    );
    match found {
        Ok(path) => path,
        Err(error) => {
            tracing::debug!(command, error = %error, "could not resolve the command; starting it as given");
            PathBuf::from(command)
        },
    }
}

#[cfg(not(windows))]
fn resolve_program(command: &str, _env: &BTreeMap<String, String>, _cwd: Option<&Path>) -> PathBuf {
    PathBuf::from(command)
}

#[cfg(windows)]
fn kill_tree(pid: Option<u32>) {
    use std::os::windows::process::CommandExt;

    let Some(pid) = pid else { return };
    let pid = pid.to_string();
    // Why: killing the direct child leaves the node or python process a shim
    // started still running. taskkill runs on its own; nothing waits for it, so
    // a drop never blocks.
    let started = std::process::Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    match started {
        Ok(taskkill) => drop(taskkill),
        Err(error) => tracing::debug!(error = %error, "could not start taskkill"),
    }
}

#[cfg(not(windows))]
const fn kill_tree(_pid: Option<u32>) {}
