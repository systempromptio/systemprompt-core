//! Local servers: a child process speaking newline-delimited JSON-RPC on stdin
//! and stdout. Its stderr goes to tracing at debug level; the process (and on
//! Windows its whole tree) is killed when the transport goes away.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin};
use tokio::sync::Mutex;

use super::lines::{LineWriter, spawn_line_reader};
use super::{InboundSink, Transport, platform};
use crate::mcp::error::McpError;

const EXIT_GRACE: Duration = Duration::from_secs(2);

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
        let program = platform::resolve_program(local.command, local.env, local.cwd);
        let mut command = platform::command(&program);
        command
            .args(local.args)
            .envs(local.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = local.cwd {
            command.current_dir(cwd);
        }
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
            platform::end_tree(self.pid);
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
            platform::end_tree(self.pid);
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
