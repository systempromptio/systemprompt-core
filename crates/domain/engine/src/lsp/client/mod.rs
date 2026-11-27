//! One running language server: its process, the JSON-RPC connection, open
//! documents and published diagnostics.
//!
//! Outgoing frames go through a writer task so that answering a server request
//! from the reader can never wait behind a large write while the server itself
//! is blocked writing to us. The reader, writer and stderr tasks are owned by
//! the [`Client`] and aborted with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod connection;
mod documents;
mod handshake;
mod incoming;
mod io;
mod process;

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};

use parking_lot::Mutex;
use serde_json::Value;
use systemprompt_traits::{CancellationToken, OwnedTask};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, watch};
use tracing::{debug, trace};

use crate::lsp::error::LspError;
use crate::lsp::parse::ServerCapabilities;
use crate::lsp::spec::{LspTimeouts, ServerSpec};

use connection::Shared;
use documents::Document;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capability {
    Hover,
    Definition,
    References,
    DocumentSymbol,
    WorkspaceSymbol,
}

#[derive(Debug, Clone)]
pub(crate) struct ClientSettings {
    pub(crate) timeouts: LspTimeouts,
    pub(crate) name: String,
    pub(crate) version: String,
}

pub(crate) struct Client {
    shared: Arc<Shared>,
    child: Mutex<Option<Child>>,
    documents: Mutex<HashMap<PathBuf, Document>>,
    capabilities: ServerCapabilities,
    timeouts: LspTimeouts,
    tasks: Vec<OwnedTask<()>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("shared", &self.shared)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

struct Pipes {
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
}

impl Client {
    pub(crate) async fn start(
        spec: &ServerSpec,
        program: &Path,
        root: &Path,
        settings: &ClientSettings,
    ) -> Result<Self, LspError> {
        let mut child = process::server_command(program, spec, root)
            .spawn()
            .map_err(|source| LspError::Spawn {
                server: spec.id.clone(),
                source,
            })?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            if let Err(error) = child.start_kill() {
                debug!(server = %spec.id, %error, "could not kill a server without pipes");
            }
            return Err(LspError::Unavailable {
                server: spec.id.clone(),
                reason: "its standard streams could not be captured".to_owned(),
            });
        };
        let mut client = Self::connect(
            spec,
            root,
            child,
            Pipes {
                stdin,
                stdout,
                stderr,
            },
            settings,
        );
        match client.initialize(spec, settings).await {
            Ok(capabilities) => {
                client.capabilities = capabilities;
                Ok(client)
            },
            Err(error) => {
                let reason = client.failure_reason();
                let error = match error {
                    LspError::Closed { server } => LspError::Unavailable { server, reason },
                    other => other,
                };
                client.terminate().await;
                Err(error)
            },
        }
    }

    fn connect(
        spec: &ServerSpec,
        root: &Path,
        child: Child,
        pipes: Pipes,
        settings: &ClientSettings,
    ) -> Self {
        let (outgoing, frames) = mpsc::unbounded_channel();
        let (publish_events, _) = watch::channel(0u64);
        let shared = Arc::new(Shared {
            server: spec.id.clone(),
            root: root.to_path_buf(),
            settings: spec.initialization_options.clone(),
            outgoing,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
            alive: AtomicBool::new(true),
            failure: Mutex::new(None),
            stderr_tail: Mutex::new(VecDeque::new()),
            published: Mutex::new(HashMap::new()),
            publish_seq: AtomicU64::new(0),
            publish_events,
            cancel: CancellationToken::new(),
        });
        let tasks = vec![
            OwnedTask::spawn(
                "lsp-writer",
                io::write_loop(pipes.stdin, frames, Arc::clone(&shared)),
            ),
            OwnedTask::spawn(
                "lsp-reader",
                io::read_loop(pipes.stdout, Arc::clone(&shared)),
            ),
            OwnedTask::spawn(
                "lsp-stderr",
                io::stderr_loop(pipes.stderr, Arc::clone(&shared)),
            ),
        ];
        Self {
            shared,
            child: Mutex::new(Some(child)),
            documents: Mutex::new(HashMap::new()),
            capabilities: ServerCapabilities::default(),
            timeouts: settings.timeouts,
            tasks,
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.shared.is_alive()
    }

    pub(crate) fn failure_reason(&self) -> String {
        self.shared
            .failure
            .lock()
            .clone()
            .unwrap_or_else(|| "the server is not running".to_owned())
    }

    pub(crate) const fn supports(&self, capability: Capability) -> bool {
        match capability {
            Capability::Hover => self.capabilities.hover,
            Capability::Definition => self.capabilities.definition,
            Capability::References => self.capabilities.references,
            Capability::DocumentSymbol => self.capabilities.document_symbol,
            Capability::WorkspaceSymbol => self.capabilities.workspace_symbol,
        }
    }

    // JSON: protocol request params in, the raw protocol result out.
    pub(crate) async fn request(&self, method: &str, params: Value) -> Result<Value, LspError> {
        self.shared
            .request(method, params, self.timeouts.request)
            .await
    }

    pub(crate) async fn shutdown(&self) {
        if self.is_alive() {
            match self
                .shared
                .request("shutdown", Value::Null, self.timeouts.shutdown)
                .await
            {
                Ok(_) => {
                    if let Err(error) = self.shared.notify("exit", Value::Null) {
                        debug!(server = %self.shared.server, %error, "could not send exit");
                    }
                },
                Err(error) => {
                    debug!(server = %self.shared.server, %error, "shutdown request failed");
                },
            }
        }
        let child = self.child.lock().take();
        if let Some(mut child) = child {
            let exited = tokio::time::timeout(self.timeouts.shutdown, child.wait()).await;
            if !matches!(exited, Ok(Ok(_))) {
                process::kill(&mut child).await;
            }
        }
        self.shared.mark_stopped();
    }

    async fn terminate(&self) {
        let child = self.child.lock().take();
        if let Some(mut child) = child {
            process::kill(&mut child).await;
        }
        self.shared.mark_stopped();
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shared.cancel.cancel();
        for task in &self.tasks {
            task.abort();
        }
        let child = self.child.lock().take();
        if let Some(mut child) = child {
            #[cfg(windows)]
            if matches!(child.try_wait(), Ok(None))
                && let Some(pid) = child.id()
            {
                process::kill_tree_detached(pid);
            }
            if let Err(error) = child.start_kill() {
                trace!(%error, "language server already gone");
            }
        }
    }
}
