//! The per-worktree entry point: lazy server start-up, document sync,
//! diagnostics and symbol queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod queries;
mod slots;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::future::join_all;
use parking_lot::Mutex;
use tracing::{info, warn};

use crate::client::{Client, ClientSettings};
use crate::error::{LspError, chain};
use crate::paths::{self, ProgramSearch};
use crate::spec::{self, LspConfig, Resolved, ServerId};
use crate::types::{Diagnostic, DiagnosticScope, ServerState, ServerStatus};

use slots::{Slot, SlotKey};

/// Language servers for one worktree.
///
/// A server instance is keyed by server id and root. It is started the first
/// time a file needs it; if it fails to start, or exits later, it stays failed
/// with its reason and is not started again.
pub struct LspManager {
    worktree: PathBuf,
    settings: ClientSettings,
    servers: Vec<Resolved>,
    slots: Mutex<HashMap<SlotKey, Arc<Slot>>>,
    stopped: AtomicBool,
}

impl fmt::Debug for LspManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LspManager")
            .field("worktree", &self.worktree)
            .field("servers", &self.server_names())
            .field("stopped", &self.stopped.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl LspManager {
    pub fn new(worktree: impl Into<PathBuf>, config: LspConfig) -> Self {
        let worktree = paths::normalize(&worktree.into());
        let search = ProgramSearch {
            path: config.search_path.as_deref(),
            path_ext: config.path_ext.as_deref(),
            base: &worktree,
        };
        let servers =
            spec::resolve_servers(&config, |command| paths::find_program(command, &search));
        let settings = ClientSettings {
            timeouts: config.timeouts,
            name: config.client_name,
            version: config.client_version,
        };
        let manager = Self {
            worktree,
            settings,
            servers,
            slots: Mutex::new(HashMap::new()),
            stopped: AtomicBool::new(false),
        };
        info!(
            worktree = %manager.worktree.display(),
            servers = ?manager.server_names(),
            "language servers available"
        );
        manager
    }

    pub fn worktree(&self) -> &Path {
        &self.worktree
    }

    pub fn available_servers(&self) -> Vec<ServerId> {
        self.servers
            .iter()
            .map(|server| server.spec.id.clone())
            .collect()
    }

    pub fn servers_for(&self, path: &Path) -> Vec<(ServerId, PathBuf)> {
        let path = self.resolve(path);
        spec::select(&self.servers, &path, &self.worktree)
            .into_iter()
            .map(|(server, root)| (server.spec.id.clone(), root))
            .collect()
    }

    pub fn handles(&self, path: &Path) -> bool {
        let path = self.resolve(path);
        !spec::select(&self.servers, &path, &self.worktree).is_empty()
    }

    pub async fn touch(&self, path: &Path, wait_for_diagnostics: bool) -> Result<(), LspError> {
        let path = self.resolve(path);
        let text = match tokio::fs::read(&path).await {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                for client in self.running_clients() {
                    client.close_document(&path);
                }
                return Ok(());
            },
            Err(source) => return Err(LspError::Read { path, source }),
        };
        let clients = self.clients_for(&path).await?;
        let syncs = clients
            .iter()
            .map(|client| sync_one(client, &path, &text, wait_for_diagnostics));
        let mut first_error = None;
        let mut any_ok = false;
        for result in join_all(syncs).await {
            match result {
                Ok(()) => any_ok = true,
                Err(error) => {
                    warn!(path = %path.display(), error = %chain(&error), "could not sync a document");
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                },
            }
        }
        match first_error {
            Some(error) if !any_ok => Err(error),
            _ => Ok(()),
        }
    }

    pub fn diagnostics(&self, scope: DiagnosticScope<'_>) -> Vec<Diagnostic> {
        let clients = self.running_clients();
        let mut found: Vec<Diagnostic> = match scope {
            DiagnosticScope::File(path) => {
                let path = self.resolve(path);
                clients
                    .iter()
                    .flat_map(|client| client.diagnostics_for(&path))
                    .collect()
            },
            DiagnosticScope::All => clients
                .iter()
                .flat_map(|client| client.all_diagnostics())
                .collect(),
        };
        found.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.severity.cmp(&b.severity))
                .then(a.range.cmp(&b.range))
                .then(a.message.cmp(&b.message))
                .then(a.source.cmp(&b.source))
                .then(a.code.cmp(&b.code))
        });
        found.dedup();
        found
    }

    pub fn status(&self) -> Vec<ServerStatus> {
        let slots: Vec<Arc<Slot>> = self.slots.lock().values().cloned().collect();
        let mut statuses: Vec<ServerStatus> = slots
            .iter()
            .map(|slot| {
                let state = match slot.client.get() {
                    None => ServerState::Starting,
                    Some(Ok(client)) if client.is_alive() => ServerState::Ready,
                    Some(Ok(client)) => ServerState::Failed {
                        reason: client.failure_reason(),
                    },
                    Some(Err(failure)) => ServerState::Failed {
                        reason: failure.reason(),
                    },
                };
                ServerStatus {
                    id: slot.key.server.clone(),
                    root: slot.key.root.clone(),
                    state,
                }
            })
            .collect();
        statuses.sort_by(|a, b| a.id.cmp(&b.id).then(a.root.cmp(&b.root)));
        statuses
    }

    pub async fn shutdown(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let slots: Vec<Arc<Slot>> = self.slots.lock().drain().map(|(_, slot)| slot).collect();
        let clients: Vec<Arc<Client>> = slots
            .iter()
            .filter_map(|slot| slot.client.get())
            .filter_map(|started| started.as_ref().ok())
            .cloned()
            .collect();
        info!(
            worktree = %self.worktree.display(),
            count = clients.len(),
            "stopping language servers"
        );
        join_all(clients.iter().map(|client| client.shutdown())).await;
    }

    fn server_names(&self) -> Vec<&str> {
        self.servers
            .iter()
            .map(|server| server.spec.id.as_str())
            .collect()
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        paths::absolutize(&self.worktree, path)
    }

    fn running_clients(&self) -> Vec<Arc<Client>> {
        let slots: Vec<Arc<Slot>> = self.slots.lock().values().cloned().collect();
        slots
            .iter()
            .filter_map(|slot| slot.client.get())
            .filter_map(|started| started.as_ref().ok())
            .filter(|client| client.is_alive())
            .cloned()
            .collect()
    }
}

async fn sync_one(
    client: &Client,
    path: &Path,
    text: &str,
    wait_for_diagnostics: bool,
) -> Result<(), LspError> {
    let synced = client.sync_document(path, text)?;
    if wait_for_diagnostics && (synced.changed || !client.has_published(path)) {
        client.wait_for_diagnostics(path, synced).await;
    }
    Ok(())
}
