//! One directory's services bundled: its snapshot store, formatter, watcher and
//! VCS facts, with async wrappers that keep blocking file work off the
//! runtime's worker threads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use harness_tasks::{CancellationToken, OwnedTask};
use tokio::sync::broadcast;
use tokio_util::sync::DropGuard;

use crate::diff::FileDiff;
use crate::format::{FormatError, Formatter, FormatterConfig};
use crate::snapshot::{
    DiffTarget, PrunePolicy, PruneReport, SnapshotError, SnapshotId, SnapshotOptions, SnapshotStore,
};
use crate::vcs::{VcsError, VcsInfo, VcsStatus};
use crate::watch::{FileEvent, FileWatcher, WatchError, WatchOptions};

const PRUNER_TASK: &str = "workspace-snapshot-pruner";
const PRUNE_TASK: &str = "workspace-snapshot-prune";
const SNAPSHOT_TASK: &str = "workspace-snapshot";
const VCS_TASK: &str = "workspace-vcs-status";
const MIN_PRUNE_INTERVAL: Duration = Duration::from_secs(1);

/// How often a workspace prunes its snapshot store in the background, and with
/// which policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PruneSchedule {
    pub every: Duration,
    pub policy: PrunePolicy,
}

/// Everything a [`Workspace`] needs besides its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceOptions {
    pub data_dir: PathBuf,
    pub snapshots: SnapshotOptions,
    pub formatter: FormatterConfig,
    pub watch: WatchOptions,
    pub prune: Option<PruneSchedule>,
}

impl WorkspaceOptions {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            snapshots: SnapshotOptions::default(),
            formatter: FormatterConfig::default(),
            watch: WatchOptions::default(),
            prune: None,
        }
    }
}

/// A handle on one directory's services. Clones share state; background work
/// stops when the last clone is dropped.
#[derive(Clone)]
pub struct Workspace {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    snapshots: SnapshotStore,
    formatter: Formatter,
    watcher: FileWatcher,
    _tasks: DropGuard,
    _pruner: Option<OwnedTask<()>>,
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("root", &self.inner.root)
            .field("snapshots", &self.inner.snapshots.store_dir())
            .field("watcher", &self.inner.watcher)
            .finish_non_exhaustive()
    }
}

impl Workspace {
    pub fn open(
        root: impl Into<PathBuf>,
        options: WorkspaceOptions,
    ) -> Result<Self, SnapshotError> {
        let root = root.into();
        let snapshots = SnapshotStore::open(&root, &options.data_dir, options.snapshots)?;
        let formatter = Formatter::new(&root, options.formatter);
        let watcher = FileWatcher::new(options.watch);
        let cancel = CancellationToken::new();
        let pruner = options
            .prune
            .and_then(|schedule| spawn_pruner(snapshots.clone(), schedule, cancel.child_token()));
        Ok(Self {
            inner: Arc::new(Inner {
                root,
                snapshots,
                formatter,
                watcher,
                _tasks: cancel.drop_guard(),
                _pruner: pruner,
            }),
        })
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    pub fn snapshots(&self) -> &SnapshotStore {
        &self.inner.snapshots
    }

    pub fn formatter(&self) -> &Formatter {
        &self.inner.formatter
    }

    pub fn watcher(&self) -> &FileWatcher {
        &self.inner.watcher
    }

    pub async fn track(&self) -> Result<SnapshotId, SnapshotError> {
        self.blocking(SnapshotStore::track).await
    }

    pub async fn diff(
        &self,
        from: SnapshotId,
        to: DiffTarget,
    ) -> Result<Vec<FileDiff>, SnapshotError> {
        self.blocking(move |store| store.diff(&from, &to)).await
    }

    pub async fn changed_files(&self, from: SnapshotId) -> Result<Vec<PathBuf>, SnapshotError> {
        self.blocking(move |store| store.changed_files(&from)).await
    }

    pub async fn restore(&self, id: SnapshotId) -> Result<Vec<PathBuf>, SnapshotError> {
        self.blocking(move |store| store.restore(&id)).await
    }

    pub async fn revert_files(
        &self,
        id: SnapshotId,
        paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, SnapshotError> {
        self.blocking(move |store| store.revert_files(&id, &paths))
            .await
    }

    pub async fn prune(&self, policy: PrunePolicy) -> Result<PruneReport, SnapshotError> {
        self.blocking(move |store| store.prune(policy)).await
    }

    pub async fn format(&self, path: &Path) -> Result<bool, FormatError> {
        self.inner.formatter.format(path).await
    }

    pub fn watch(&self) -> Result<broadcast::Receiver<FileEvent>, WatchError> {
        let receiver = self.inner.watcher.subscribe();
        self.inner.watcher.watch(&self.inner.root)?;
        Ok(receiver)
    }

    pub fn unwatch(&self) -> bool {
        self.inner.watcher.unwatch(&self.inner.root)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileEvent> {
        self.inner.watcher.subscribe()
    }

    pub async fn vcs(&self) -> Result<Option<VcsStatus>, VcsError> {
        let root = self.inner.root.clone();
        match OwnedTask::spawn_blocking(VCS_TASK, move || VcsInfo::status(&root))
            .join()
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(%error, "vcs status task did not complete");
                Ok(None)
            },
        }
    }

    pub fn vcs_info(&self) -> Result<Option<VcsInfo>, VcsError> {
        VcsInfo::discover(&self.inner.root)
    }

    async fn blocking<T, F>(&self, work: F) -> Result<T, SnapshotError>
    where
        T: Send + 'static,
        F: FnOnce(&SnapshotStore) -> Result<T, SnapshotError> + Send + 'static,
    {
        let store = self.inner.snapshots.clone();
        OwnedTask::spawn_blocking(SNAPSHOT_TASK, move || work(&store))
            .join()
            .await
            .map_err(SnapshotError::Task)?
    }
}

fn spawn_pruner(
    store: SnapshotStore,
    schedule: PruneSchedule,
    cancel: CancellationToken,
) -> Option<OwnedTask<()>> {
    if tokio::runtime::Handle::try_current().is_err() {
        tracing::warn!("no async runtime: snapshot pruning will not run in the background");
        return None;
    }
    let every = schedule.every.max(MIN_PRUNE_INTERVAL);
    let pruning = prune_periodically(store, schedule.policy, every, cancel);
    Some(OwnedTask::spawn(PRUNER_TASK, pruning))
}

async fn prune_periodically(
    store: SnapshotStore,
    policy: PrunePolicy,
    every: Duration,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            () = tokio::time::sleep(every) => {}
        }
        let store = store.clone();
        match OwnedTask::spawn_blocking(PRUNE_TASK, move || store.prune(policy))
            .join()
            .await
        {
            Ok(Ok(report)) => tracing::debug!(
                snapshots = report.snapshots_removed,
                objects = report.objects_removed,
                "background snapshot prune finished"
            ),
            Ok(Err(error)) => tracing::warn!(%error, "background snapshot prune failed"),
            Err(error) => tracing::warn!(%error, "background snapshot prune task failed"),
        }
    }
}
