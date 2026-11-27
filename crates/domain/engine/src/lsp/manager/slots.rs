//! Server instances keyed by server and root: started once on first use, failed
//! for good on error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures_util::future::join_all;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

use super::LspManager;
use crate::lsp::client::Client;
use crate::lsp::error::{LspError, chain};
use crate::lsp::spec::{self, MissingCommand, Resolved, ServerId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SlotKey {
    pub(super) server: ServerId,
    pub(super) root: PathBuf,
}

#[derive(Debug)]
pub(super) struct Slot {
    pub(super) key: SlotKey,
    pub(super) client: OnceCell<Result<Arc<Client>, StartFailure>>,
}

#[derive(Debug, Clone)]
pub(super) enum StartFailure {
    Missing(MissingCommand),
    Failed(Arc<LspError>),
}

impl StartFailure {
    pub(super) fn reason(&self) -> String {
        match self {
            Self::Missing(missing) => missing.to_string(),
            Self::Failed(error) => chain(&**error),
        }
    }
}

impl LspManager {
    pub(super) async fn clients_for(&self, path: &Path) -> Result<Vec<Arc<Client>>, LspError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(LspError::Stopped);
        }
        let matches = spec::select(&self.servers, path, &self.worktree);
        if matches.is_empty() {
            return Err(LspError::NoServer {
                path: path.to_path_buf(),
            });
        }
        let starts = matches
            .into_iter()
            .map(|(server, root)| self.client(server, root));
        let mut clients = Vec::new();
        let mut first_failure = None;
        for result in join_all(starts).await {
            match result {
                Ok(client) => clients.push(client),
                Err(failure) => {
                    if first_failure.is_none() {
                        first_failure = Some(failure);
                    }
                },
            }
        }
        match first_failure {
            Some(failure) if clients.is_empty() => Err(failure),
            _ => Ok(clients),
        }
    }

    async fn client(&self, server: &Resolved, root: PathBuf) -> Result<Arc<Client>, LspError> {
        let key = SlotKey {
            server: server.spec.id.clone(),
            root,
        };
        let slot = Arc::clone(self.slots.lock().entry(key.clone()).or_insert_with(|| {
            Arc::new(Slot {
                key,
                client: OnceCell::new(),
            })
        }));
        let started = slot
            .client
            .get_or_init(|| self.start(server, &slot.key.root))
            .await;
        let reason = match started {
            Ok(client) if client.is_alive() => return Ok(Arc::clone(client)),
            Ok(client) => client.failure_reason(),
            Err(failure) => failure.reason(),
        };
        Err(LspError::Unavailable {
            server: server.spec.id.clone(),
            reason,
        })
    }

    async fn start(&self, server: &Resolved, root: &Path) -> Result<Arc<Client>, StartFailure> {
        let id = &server.spec.id;
        let program = match &server.program {
            Ok(program) => program,
            Err(missing) => {
                warn!(server = %id, reason = %missing, "language server cannot be started");
                return Err(StartFailure::Missing(missing.clone()));
            },
        };
        info!(
            server = %id,
            root = %root.display(),
            program = %program.display(),
            "starting language server"
        );
        match Client::start(&server.spec, program, root, &self.settings).await {
            Ok(client) => {
                debug!(server = %id, root = %root.display(), "language server ready");
                Ok(Arc::new(client))
            },
            Err(error) => {
                let reason = chain(&error);
                warn!(server = %id, root = %root.display(), %reason, "language server failed to start");
                Err(StartFailure::Failed(Arc::new(error)))
            },
        }
    }
}
