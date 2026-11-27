//! Per-directory state: the loaded configuration with the admitted
//! extensions' agents and commands (reloaded when its inputs or the
//! extensions' contributions change), the worktree's workspace, a language
//! server manager, the MCP servers and the built-in tools. Created on first
//! use and cached by the engine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod convert;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::Loaded;
use crate::lsp::LspManager;
use crate::mcp::{ManagerConfig, McpManager, StatusMap};
use systemprompt_traits::CancellationToken;
use crate::tools::{ShellSpec, Tool};
use crate::workspace::Workspace;
use parking_lot::RwLock;
use tokio::sync::{Mutex, broadcast, watch};

use convert::{builtin_agents, builtin_tools, lsp_config, mcp_servers};
pub(crate) use convert::{formatter_config, prompt_shell};

use crate::run::error::EngineError;
use crate::run::events::{EngineEvent, publish};
use crate::run::governed::Contributions;
use crate::run::governor::McpSpec;
use crate::run::inner::Inner;
use crate::run::session::commands::is_builtin_command;

pub(crate) struct DirState {
    pub(crate) directory: PathBuf,
    config: RwLock<Arc<Loaded>>,
    revision: AtomicU64,
    reload: Mutex<()>,
    pub(crate) workspace: Workspace,
    pub(crate) lsp: Arc<LspManager>,
    pub(crate) mcp: McpManager,
    pub(crate) shell: ShellSpec,
    pub(crate) builtin: Vec<Arc<dyn Tool>>,
}

impl DirState {
    pub(crate) async fn open(inner: &Inner, directory: &Path) -> Result<Self, EngineError> {
        let contributions = inner.governed.current();
        let loaded = load(inner, directory, &contributions).await?;
        let root = loaded.worktree.root.clone();
        let workspace = inner.workspace_for(&root, &loaded.settings)?;
        let lsp_settings = lsp_config(&loaded.settings, inner);
        let lsp_root = root.clone();
        let lsp = tokio::task::spawn_blocking(move || LspManager::new(lsp_root, lsp_settings))
            .await
            .map_err(EngineError::Task)?;
        let mcp = McpManager::new(ManagerConfig::new(inner.client_info.clone()))?;
        mcp.reconcile(mcp_servers(&loaded, &contributions.mcp_servers));
        let shell = ShellSpec::detect(loaded.settings.shell.as_deref().map(Path::new));
        let builtin = builtin_tools(&shell, &inner.jobs, &inner.todos);
        let feed = mcp.subscribe();
        let events = inner.events.clone();
        let watched = directory.to_path_buf();
        inner
            .tasks
            .spawn_cancellable("mcp status feed", move |cancel| {
                forward_mcp_changes(feed, events, watched, cancel)
            });
        tracing::debug!(
            directory = %directory.display(),
            root = %root.display(),
            shell = ?shell.kind,
            "directory opened"
        );
        Ok(Self {
            directory: directory.to_path_buf(),
            config: RwLock::new(Arc::new(loaded)),
            revision: AtomicU64::new(contributions.revision),
            reload: Mutex::new(()),
            workspace,
            lsp: Arc::new(lsp),
            mcp,
            shell,
            builtin,
        })
    }

    pub(crate) fn config(&self) -> Arc<Loaded> {
        Arc::clone(&self.config.read())
    }

    pub(crate) async fn current(&self, inner: &Inner) -> Result<Arc<Loaded>, EngineError> {
        let _guard = self.reload.lock().await;
        let current = self.config();
        let contributions = inner.governed.current();
        let probe = Arc::clone(&current);
        let stale = tokio::task::spawn_blocking(move || probe.is_stale())
            .await
            .map_err(EngineError::Task)?;
        if !stale && self.revision.load(Ordering::Acquire) == contributions.revision {
            return Ok(current);
        }
        let fresh = Arc::new(load(inner, &self.directory, &contributions).await?);
        *self.config.write() = Arc::clone(&fresh);
        self.revision
            .store(contributions.revision, Ordering::Release);
        self.mcp
            .reconcile(mcp_servers(&fresh, &contributions.mcp_servers));
        tracing::debug!(directory = %self.directory.display(), "configuration reloaded");
        Ok(fresh)
    }

    pub(crate) fn reconcile_mcp(&self, offered: &[McpSpec]) {
        self.mcp.reconcile(mcp_servers(&self.config(), offered));
    }

    pub(crate) async fn shutdown(&self) {
        self.mcp.shutdown().await;
        self.lsp.shutdown().await;
    }
}

async fn forward_mcp_changes(
    mut feed: watch::Receiver<StatusMap>,
    events: broadcast::Sender<EngineEvent>,
    directory: PathBuf,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            changed = feed.changed() => {
                if changed.is_err() {
                    break;
                }
                publish(&events, EngineEvent::McpChanged { directory: directory.clone() });
            }
        }
    }
}

async fn load(
    inner: &Inner,
    directory: &Path,
    contributions: &Contributions,
) -> Result<Loaded, EngineError> {
    let paths = inner.paths.clone();
    let layout = inner.layout.clone();
    let dir = directory.to_path_buf();
    let mut builtins = builtin_agents(&paths);
    for contributed in &contributions.agents {
        let name = &contributed.item.name;
        if builtins.iter().any(|agent| &agent.name == name) {
            tracing::warn!(extension = %contributed.extension, agent = %name, "an extension agent shares a built-in agent's name; not offered");
            continue;
        }
        builtins.push(contributed.item.clone());
    }
    let mut loaded = tokio::task::spawn_blocking(move || {
        Loaded::load(&paths, &layout, &dir, builtins).map_err(|source| EngineError::Config {
            dir: dir.clone(),
            source: Box::new(source),
        })
    })
    .await
    .map_err(EngineError::Task)??;
    for contributed in &contributions.commands {
        let name = &contributed.item.name;
        if is_builtin_command(name) || loaded.commands.contains_key(name) {
            tracing::warn!(extension = %contributed.extension, command = %name, "an extension command shares a command's name; not offered");
            continue;
        }
        loaded
            .commands
            .insert(name.clone(), contributed.item.clone());
    }
    Ok(loaded)
}
