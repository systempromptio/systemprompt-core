//! The handle the Drive shares through `SpContext::handles`: the applied
//! instance's Drive client, withheld while no instance that enables the Drive
//! is applied, and the local folder sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_device::client::SpClient;
use sp_protocol::ids::AccountId;
use tokio::sync::watch;

use systemprompt_drive::client::DriveClient;
use systemprompt_drive::sync::DriveSync;

/// The Drive is withheld: no applied instance enables it for this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no applied instance enables the Drive")]
pub struct DriveWithheld;

/// The Drive as the shell, the agent tools and other extensions reach it.
///
/// Cheap to clone; every clone sees one applied instance and one sync.
/// `client` answers the applied instance's Drive client, or [`DriveWithheld`]
/// while no instance that enables the Drive is applied (the extension's
/// manifest applier follows and withholds it), so an account switch is
/// followed on the next call. `sync` is the local folder sync that the
/// [`DriveFollower`](super::DriveFollower) service starts and stops.
#[derive(Clone)]
pub struct DriveHandle {
    shared: Arc<Shared>,
}

struct Shared {
    target: watch::Sender<Option<Target>>,
    sync: Arc<DriveSync>,
}

#[derive(Clone)]
pub(crate) struct Target {
    pub(crate) account: AccountId,
    pub(crate) instance: SpClient,
    pub(crate) client: DriveClient,
}

impl DriveHandle {
    pub fn new() -> Self {
        Self::with_sync(DriveSync::default())
    }

    pub fn with_sync(sync: DriveSync) -> Self {
        let (target, _) = watch::channel(None);
        Self {
            shared: Arc::new(Shared {
                target,
                sync: Arc::new(sync),
            }),
        }
    }

    pub fn client(&self) -> Result<DriveClient, DriveWithheld> {
        self.shared
            .target
            .borrow()
            .as_ref()
            .map(|target| target.client.clone())
            .ok_or(DriveWithheld)
    }

    pub fn is_withheld(&self) -> bool {
        self.shared.target.borrow().is_none()
    }

    pub fn sync(&self) -> &Arc<DriveSync> {
        &self.shared.sync
    }

    pub(crate) fn follow(&self, account: AccountId, instance: SpClient, client: DriveClient) {
        self.shared.target.send_if_modified(|current| {
            let moved = current.as_ref().is_none_or(|held| {
                held.account != account || held.instance.base() != instance.base()
            });
            *current = Some(Target {
                account,
                instance,
                client,
            });
            moved
        });
    }

    pub(crate) fn withhold(&self) {
        self.shared
            .target
            .send_if_modified(|current| current.take().is_some());
    }

    pub(crate) fn targets(&self) -> watch::Receiver<Option<Target>> {
        self.shared.target.subscribe()
    }
}

impl Default for DriveHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DriveHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriveHandle")
            .field("withheld", &self.is_withheld())
            .field("sync", &self.shared.sync)
            .finish()
    }
}
