//! [`SyncEngine`]: the public handle, built once per process from [`SyncDeps`]. It owns the supervisor that follows
//! the accounts and the heartbeat; both run as tasks of a set it aborts when
//! dropped and drains on shutdown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use harness_tasks::OwnedTask;
use parking_lot::Mutex;
use systemprompt_device::accounts::{Accounts, AccountsSnapshot};
use crate::extension::ManifestAppliers;
use crate::governance::Governance;
use crate::projection::Applier;
use sp_protocol::view::SyncStatus;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::sync::account_loop::LoopContext;
use crate::sync::cadence::{Clock, SystemClock};
use crate::sync::error::SyncError;
use crate::sync::heartbeat::{Heartbeat, HeartbeatState, Reporter};
use crate::sync::ports::Store;
use crate::sync::supervisor::Supervisor;
use crate::sync::target::{LiveTarget, Target};

/// What the sync engine is built from.
///
/// `appliers` are the extensions' manifest appliers, governance's among
/// them; `governance` is the same instance, kept for device evidence, usage
/// totals and session bindings. `state_dir` holds the receipts outboxes,
/// `cache_dir` the plugin bundle file cache; `release_version` is this build's
/// own version, reported by the heartbeat; `cancel` stops every task.
pub struct SyncDeps {
    pub accounts: Accounts,
    pub governance: Arc<Governance>,
    pub appliers: ManifestAppliers,
    pub applier: Applier,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub release_version: String,
    pub cancel: CancellationToken,
}

impl fmt::Debug for SyncDeps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncDeps")
            .field("state_dir", &self.state_dir)
            .field("cache_dir", &self.cache_dir)
            .field("release_version", &self.release_version)
            .finish_non_exhaustive()
    }
}

/// The live sync of the active systemprompt account. Cheap to clone; every
/// clone is the same engine.
#[derive(Clone)]
pub struct SyncEngine {
    shared: Arc<Shared>,
}

struct Shared {
    status: Arc<watch::Sender<SyncStatus>>,
    heartbeat: Arc<watch::Sender<HeartbeatState>>,
    focused: watch::Sender<bool>,
    sync_now: watch::Sender<u64>,
    cancel: CancellationToken,
    tasks: Mutex<Vec<OwnedTask<()>>>,
}

impl fmt::Debug for SyncEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncEngine")
            .field("status", &*self.shared.status.borrow())
            .finish_non_exhaustive()
    }
}

/// The collaborators [`SyncEngine::launch`] runs over: the accounts store, the
/// apply target, the accounts snapshots it follows and the clock its statuses
/// and backoffs read.
pub struct Launch<S: Store, T: Target> {
    pub store: S,
    pub target: T,
    pub snapshots: watch::Receiver<AccountsSnapshot>,
    pub clock: Arc<dyn Clock>,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub device_label: String,
    pub release_version: String,
    pub cancel: CancellationToken,
}

impl<S: Store, T: Target> fmt::Debug for Launch<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Launch")
            .field("state_dir", &self.state_dir)
            .field("cache_dir", &self.cache_dir)
            .field("device_label", &self.device_label)
            .field("release_version", &self.release_version)
            .finish_non_exhaustive()
    }
}

impl SyncEngine {
    pub fn start(deps: SyncDeps) -> Result<Self, SyncError> {
        let runtime = Handle::try_current().map_err(SyncError::NoRuntime)?;
        let SyncDeps {
            accounts,
            governance,
            appliers,
            applier,
            state_dir,
            cache_dir,
            release_version,
            cancel,
        } = deps;
        let launch = Launch {
            store: accounts.clone(),
            target: LiveTarget::new(applier, appliers, governance, accounts.clone()),
            snapshots: accounts.subscribe(),
            clock: Arc::new(SystemClock),
            state_dir,
            cache_dir,
            device_label: accounts.device_name().to_owned(),
            release_version,
            cancel,
        };
        Ok(Self::launch(launch, &runtime))
    }

    pub fn launch<S: Store, T: Target>(launch: Launch<S, T>, runtime: &Handle) -> Self {
        let cancel = launch.cancel.child_token();
        let status = Arc::new(watch::Sender::new(SyncStatus::Idle));
        let heartbeat = Arc::new(watch::Sender::new(HeartbeatState::default()));
        let focused = watch::Sender::new(true);
        let sync_now = watch::Sender::new(0u64);
        let context = LoopContext {
            store: launch.store.clone(),
            target: launch.target.clone(),
            status: Arc::clone(&status),
            focus: focused.subscribe(),
            sync_now: sync_now.subscribe(),
            clock: launch.clock,
            state_dir: launch.state_dir,
            cache_dir: launch.cache_dir,
            device_label: launch.device_label,
        };
        let supervisor = Supervisor {
            context,
            snapshots: launch.snapshots.clone(),
        };
        let beats = Heartbeat {
            store: launch.store,
            target: launch.target,
            reporter: Reporter::new(&launch.release_version),
            snapshots: launch.snapshots,
            state: Arc::clone(&heartbeat),
        };
        let entered = runtime.enter();
        let tasks = vec![
            OwnedTask::spawn("sync-supervisor", supervisor.run(cancel.clone())),
            OwnedTask::spawn("sync-heartbeat", beats.run(cancel.clone())),
        ];
        drop(entered);
        tracing::info!("sync engine started");
        let tasks = Mutex::new(tasks);
        Self {
            shared: Arc::new(Shared {
                status,
                heartbeat,
                focused,
                sync_now,
                cancel,
                tasks,
            }),
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<SyncStatus> {
        self.shared.status.subscribe()
    }

    #[must_use]
    pub fn status(&self) -> SyncStatus {
        self.shared.status.borrow().clone()
    }

    #[must_use]
    pub fn subscribe_heartbeat(&self) -> watch::Receiver<HeartbeatState> {
        self.shared.heartbeat.subscribe()
    }

    #[must_use]
    pub fn heartbeat(&self) -> HeartbeatState {
        self.shared.heartbeat.borrow().clone()
    }

    pub fn set_focused(&self, focused: bool) {
        self.shared.focused.send_if_modified(|current| {
            let changed = *current != focused;
            *current = focused;
            changed
        });
    }

    pub fn sync_now(&self) {
        self.shared
            .sync_now
            .send_modify(|asked| *asked = asked.wrapping_add(1));
    }

    pub async fn shutdown(&self) {
        self.shared.cancel.cancel();
        let tasks = std::mem::take(&mut *self.shared.tasks.lock());
        for task in tasks {
            if let Err(error) = task.join().await {
                tracing::warn!(error = %error, "a sync task ended abnormally");
            }
        }
        tracing::info!("sync engine stopped");
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
