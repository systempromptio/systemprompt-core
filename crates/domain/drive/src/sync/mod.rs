//! [`DriveSync`]: the active account's Drive mirrored into a local folder.
//!
//! The folder is the caller's choice; [`default_drive_root`] names
//! `<Documents>/<product> Drive` and [`account_root`] one subfolder per
//! instance under it, so two accounts never share a folder (the index records
//! the instance it mirrors and a folder holding another instance's index is
//! refused). One owned background task runs the cycles: at start, on every poll
//! ([`SyncOptions::poll_interval`]), on a debounced change in the folder, and
//! on [`DriveSync::sync_now`]; cycles never overlap. Switching account stops
//! the task and starts one on the new folder.
//!
//! The sync talks to the instance's Drive sync feed (`/v1/bridge/drive/`),
//! whose credential is a sync token traded for the device token of the sign-in
//! ([`systemprompt_device::client::DeviceTokenSource`]); the account's session JWT is never used
//! for it. Conflict policy: a local edit is never overwritten; when the
//! instance moved on, the local file is kept beside the head as `<name>
//! (conflict from <device>).<ext>` and listed in the status until the user
//! removes the copy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod account;
mod cycle;
mod deletes;
mod disk;
mod error;
mod feed;
mod folder_watch;
mod index;
mod names;
mod place;
mod pull;
mod push;
mod restore;
mod root;
mod runner;
mod scan;
mod status;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use harness_tasks::OwnedTask;
use tokio::sync::{Notify, watch};
use tokio_util::sync::CancellationToken;

pub use account::{SyncAccount, SyncOptions};
pub use error::SyncError;
pub use index::{
    ConflictNotice, DeleteNotice, HeldChoice, HeldDeletes, HeldReason, IndexEntry, PendingDelete,
    SentForReview, SyncIndex,
};
pub use names::{PathRefusal, conflict_name, is_ignored};
pub use root::{account_root, default_drive_root};
pub use status::{
    ActivityItem, ActivityKind, DriveCounts, DriveStatus, SyncPhase, SyncProblem, SyncProblemKind,
};

use cycle::Summary;
use feed::FeedClient;
use root::{SyncRoot, prepare_root};
use runner::Runner;

const SYNC_TASK: &str = "drive-sync";

/// The local folder sync of the active account. Dropping it stops the
/// background task.
pub struct DriveSync {
    shared: Arc<Shared>,
}

struct Shared {
    status: watch::Sender<DriveStatus>,
    wake: Arc<Notify>,
    paused: AtomicBool,
    cycle: tokio::sync::Mutex<()>,
    running: parking_lot::Mutex<Option<Running>>,
    options: SyncOptions,
}

struct Running {
    cancel: CancellationToken,
    task: OwnedTask<()>,
    root: SyncRoot,
}

impl fmt::Debug for DriveSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriveSync")
            .field("options", &self.shared.options)
            .finish_non_exhaustive()
    }
}

impl Default for DriveSync {
    fn default() -> Self {
        Self::new(SyncOptions::default())
    }
}

impl DriveSync {
    #[must_use]
    pub fn new(options: SyncOptions) -> Self {
        Self {
            shared: Arc::new(Shared {
                status: watch::Sender::new(DriveStatus::default()),
                wake: Arc::new(Notify::new()),
                paused: AtomicBool::new(false),
                cycle: tokio::sync::Mutex::new(()),
                running: parking_lot::Mutex::new(None),
                options,
            }),
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<DriveStatus> {
        self.shared.status.subscribe()
    }

    #[must_use]
    pub fn status(&self) -> DriveStatus {
        self.shared.status.borrow().clone()
    }

    #[must_use]
    pub fn root(&self) -> Option<PathBuf> {
        self.shared
            .running
            .lock()
            .as_ref()
            .map(|running| running.root.dir().to_path_buf())
    }

    pub async fn start(&self, account: SyncAccount) -> Result<(), SyncError> {
        self.stop().await;
        let root = prepare_root(&account.root, account.instance.base())?;
        let summary = match cycle::read_summary(&root) {
            Ok(summary) => summary,
            Err(error) => {
                tracing::warn!(error = %error, "the Drive index could not be read before the first cycle");
                Summary::default()
            },
        };
        let paused = self.shared.paused.load(Ordering::Acquire);
        let dir = root.dir().to_path_buf();
        self.shared.status.send_modify(|status| {
            *status = DriveStatus::default();
            status.phase = if paused {
                SyncPhase::Paused
            } else {
                SyncPhase::Idle
            };
            status.root = Some(dir);
            status.absorb(summary);
        });
        let host = account
            .host_name
            .or_else(|| {
                hostname::get()
                    .ok()
                    .and_then(|name| name.into_string().ok())
            })
            .map_or_else(|| names::host_label(""), |name| names::host_label(&name));
        let runner = Runner {
            shared: Arc::clone(&self.shared),
            feed: FeedClient::new(account.instance.base().to_owned(), account.http),
            credentials: account.credentials,
            root: root.clone(),
            device_id: account.device_id,
            host,
            sync_token: None,
            disabled_at: None,
            watch: None,
        };
        let cancel = CancellationToken::new();
        let task = OwnedTask::spawn(SYNC_TASK, runner.run(cancel.clone()));
        tracing::info!(root = %root.dir().display(), "the Drive sync started");
        *self.shared.running.lock() = Some(Running { cancel, task, root });
        Ok(())
    }

    pub async fn stop(&self) {
        let running = self.shared.running.lock().take();
        let Some(running) = running else {
            return;
        };
        running.cancel.cancel();
        if let Err(error) = running.task.join().await {
            tracing::error!(error = %error, "the Drive sync task ended abnormally");
        }
        self.shared
            .status
            .send_modify(|status| *status = DriveStatus::default());
        tracing::info!("the Drive sync stopped");
    }

    pub async fn switch_account(&self, account: Option<SyncAccount>) -> Result<(), SyncError> {
        self.stop().await;
        match account {
            Some(account) => self.start(account).await,
            None => Ok(()),
        }
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
        self.shared.status.send_if_modified(|status| {
            let active = status.phase != SyncPhase::Off;
            if active {
                status.phase = SyncPhase::Paused;
            }
            active
        });
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Release);
        self.shared.status.send_if_modified(|status| {
            let paused = status.phase == SyncPhase::Paused;
            if paused {
                status.phase = SyncPhase::Idle;
            }
            paused
        });
        self.shared.wake.notify_one();
    }

    pub fn sync_now(&self) {
        self.shared.wake.notify_one();
    }

    pub async fn choose_held_deletes(&self, choice: HeldChoice) -> Result<(), SyncError> {
        let root = self
            .shared
            .running
            .lock()
            .as_ref()
            .map(|running| running.root.clone());
        let Some(root) = root else {
            return Ok(());
        };
        {
            let _serial = self.shared.cycle.lock().await;
            let path = root.index_path();
            let mut index = SyncIndex::load(&path)?;
            if index.held_deletes.is_none() {
                return Ok(());
            }
            index.held_choice = Some(choice);
            index.save(&path)?;
        }
        self.shared.wake.notify_one();
        Ok(())
    }
}

impl Drop for DriveSync {
    fn drop(&mut self) {
        let running = self.shared.running.lock().take();
        if let Some(running) = running {
            running.cancel.cancel();
        }
    }
}
