//! The shared state behind every [`Engine`](crate::run::Engine) clone: options,
//! caches, the running sessions, pending asks, background jobs and the one task
//! set everything the engine starts belongs to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::config::{Layout, Loaded, Paths, Ruleset, Settings};
use crate::llm::{ModelSpec, Transport};
use crate::mcp::ClientInfo;
use crate::store::{Session, SessionId, Store, StoreError};
use harness_tasks::{BackgroundTasks, CancellationToken};
use crate::tools::{BackgroundJobs, TodoSink};
use crate::workspace::{PrunePolicy, PruneSchedule, Workspace, WorkspaceOptions};
use parking_lot::{Mutex, RwLock};
use tokio::sync::broadcast;

use crate::run::db::Db;
use crate::run::directory::{DirState, formatter_config};
use crate::run::error::EngineError;
use crate::run::events::{EngineEvent, publish};
use crate::run::governed::Governed;
use crate::run::options::EngineOptions;
use crate::run::session::Runs;
use crate::run::tools::asks::Asks;
use crate::run::tools::hooks::{JobNotices, StoreTodos};

const EVENT_CAPACITY: usize = 512;
const SNAPSHOT_PRUNE_EVERY: Duration = Duration::from_hours(1);

struct ModelCache {
    generation: u64,
    models: Vec<ModelSpec>,
}

pub(crate) struct Inner {
    pub(crate) paths: Paths,
    pub(crate) layout: Layout,
    pub(crate) store: Store,
    pub(crate) db: Db,
    pub(crate) governed: Governed,
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) client_info: ClientInfo,
    pub(crate) search_path: Option<OsString>,
    pub(crate) path_ext: Option<OsString>,
    pub(crate) lsp_client_name: Option<String>,
    pub(crate) events: broadcast::Sender<EngineEvent>,
    pub(crate) runs: Runs,
    pub(crate) asks: Asks,
    pub(crate) jobs: BackgroundJobs,
    pub(crate) todos: Arc<dyn TodoSink>,
    pub(crate) tasks: BackgroundTasks,
    pub(crate) closing: CancellationToken,
    dirs: tokio::sync::Mutex<HashMap<PathBuf, Arc<DirState>>>,
    workspaces: Mutex<HashMap<PathBuf, Workspace>>,
    rules: Mutex<HashMap<SessionId, Ruleset>>,
    models: RwLock<ModelCache>,
    mcp_revision: AtomicU64,
}

impl Inner {
    pub(crate) fn new(options: EngineOptions) -> Arc<Self> {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let db = Db::new(options.store.clone());
        let todos: Arc<dyn TodoSink> = Arc::new(StoreTodos::new(db.clone()));
        let tasks = BackgroundTasks::new();
        let governed = Governed::new(options.governor, options.extensions, options.paths.clone());
        let generation = governed.generation();
        let models = governed.models();
        let revision = governed.current().revision;
        let inner = Arc::new(Self {
            paths: options.paths,
            layout: options.layout,
            store: options.store,
            db,
            governed,
            transport: options.transport,
            client_info: options.client_info,
            search_path: options.search_path,
            path_ext: options.path_ext,
            lsp_client_name: options.lsp_client_name,
            events,
            runs: Runs::default(),
            asks: Asks::default(),
            jobs: BackgroundJobs::with_tasks(tasks.clone()),
            todos,
            tasks,
            closing: CancellationToken::new(),
            dirs: tokio::sync::Mutex::new(HashMap::new()),
            workspaces: Mutex::new(HashMap::new()),
            rules: Mutex::new(HashMap::new()),
            models: RwLock::new(ModelCache { generation, models }),
            mcp_revision: AtomicU64::new(revision),
        });
        inner
            .jobs
            .set_listener(Arc::new(JobNotices::new(Arc::downgrade(&inner))));
        inner
    }

    pub(crate) fn emit(&self, event: EngineEvent) {
        publish(&self.events, event);
    }

    pub(crate) fn models(&self) -> Vec<ModelSpec> {
        self.refresh_models(self.governed.generation());
        self.models.read().models.clone()
    }

    fn refresh_models(&self, generation: u64) {
        if self.models.read().generation == generation {
            return;
        }
        let models = self.governed.models();
        let count = models.len();
        *self.models.write() = ModelCache { generation, models };
        tracing::info!(generation, models = count, "model list changed");
        self.emit(EngineEvent::ModelsChanged);
    }

    pub(crate) fn resolve_model(&self, id: Option<&str>) -> Result<ModelSpec, EngineError> {
        let models = self.models();
        if models.is_empty() {
            return Err(EngineError::NoModels);
        }
        match id.map(str::trim).filter(|id| !id.is_empty()) {
            Some(id) => models
                .into_iter()
                .find(|model| model.id == id)
                .ok_or_else(|| EngineError::ModelNotOffered {
                    model: id.to_owned(),
                }),
            None => models.into_iter().next().ok_or(EngineError::NoModels),
        }
    }

    pub(crate) fn is_offered(&self, id: &str) -> bool {
        self.models().iter().any(|model| model.id == id)
    }

    pub(crate) async fn check_generation(&self) {
        let generation = self.governed.generation();
        self.refresh_models(generation);
        let contributions = self.governed.current();
        let revision = contributions.revision;
        if self.mcp_revision.swap(revision, Ordering::AcqRel) == revision {
            return;
        }
        let dirs: Vec<Arc<DirState>> = self.dirs.lock().await.values().cloned().collect();
        for dir in &dirs {
            dir.reconcile_mcp(&contributions.mcp_servers);
        }
        tracing::info!(
            generation,
            revision,
            servers = contributions.mcp_servers.len(),
            directories = dirs.len(),
            "managed MCP servers reconciled"
        );
    }

    pub(crate) async fn dir(&self, directory: &Path) -> Result<Arc<DirState>, EngineError> {
        if !directory.is_absolute() {
            return Err(EngineError::RelativeDirectory {
                path: directory.to_path_buf(),
            });
        }
        if self.closing.is_cancelled() {
            return Err(EngineError::ShutDown);
        }
        let mut dirs = self.dirs.lock().await;
        if let Some(dir) = dirs.get(directory) {
            return Ok(Arc::clone(dir));
        }
        let opened = Arc::new(DirState::open(self, directory).await?);
        dirs.insert(directory.to_path_buf(), Arc::clone(&opened));
        drop(dirs);
        Ok(opened)
    }

    pub(crate) async fn close_dirs(&self) {
        let dirs: Vec<Arc<DirState>> = self.dirs.lock().await.drain().map(|(_, dir)| dir).collect();
        for dir in dirs {
            dir.shutdown().await;
        }
    }

    pub(crate) async fn config(&self, directory: &Path) -> Result<Arc<Loaded>, EngineError> {
        let dir = self.dir(directory).await?;
        dir.current(self).await
    }

    pub(crate) fn workspace_for(
        &self,
        root: &Path,
        settings: &Settings,
    ) -> Result<Workspace, EngineError> {
        let mut workspaces = self.workspaces.lock();
        if let Some(workspace) = workspaces.get(root) {
            return Ok(workspace.clone());
        }
        let mut options = WorkspaceOptions::new(self.paths.data.join("snapshots"));
        options.formatter = formatter_config(settings, self.search_path.clone());
        options.prune = Some(PruneSchedule {
            every: SNAPSHOT_PRUNE_EVERY,
            policy: PrunePolicy::default(),
        });
        let workspace = Workspace::open(root, options)?;
        workspaces.insert(root.to_path_buf(), workspace.clone());
        drop(workspaces);
        Ok(workspace)
    }

    pub(crate) async fn session(&self, id: &SessionId) -> Result<Session, EngineError> {
        let key = id.clone();
        self.db
            .run(move |store| store.get_session(&key))
            .await?
            .ok_or_else(|| EngineError::SessionNotFound { id: id.clone() })
    }

    pub(crate) async fn session_rules(&self, id: &SessionId) -> Result<Ruleset, EngineError> {
        let cached = self.rules.lock().get(id).cloned();
        if let Some(rules) = cached {
            return Ok(rules);
        }
        let session = self.session(id).await?;
        let rules = session.permission.map_or_else(Ruleset::default, |value| {
            serde_json::from_value::<Ruleset>(value).unwrap_or_else(|error| {
                tracing::warn!(session = %id, error = %error, "stored approvals unreadable");
                Ruleset::default()
            })
        });
        self.rules.lock().insert(id.clone(), rules.clone());
        Ok(rules)
    }

    pub(crate) async fn add_session_rules(
        &self,
        id: &SessionId,
        added: &Ruleset,
    ) -> Result<(), EngineError> {
        let mut rules = self.session_rules(id).await?;
        rules.extend(added);
        self.rules.lock().insert(id.clone(), rules.clone());
        let value = serde_json::to_value(&rules).map_err(StoreError::from)?;
        let key = id.clone();
        self.db
            .run(move |store| {
                store.update_session(&key, |session| session.permission = Some(value))
            })
            .await?;
        Ok(())
    }

    pub(crate) fn forget_session(&self, id: &SessionId) {
        self.rules.lock().remove(id);
    }
}

pub(crate) fn project_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
