//! What a Drive sync is started with: the account, its device-token source and
//! the loop's timings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_device::client::{DeviceTokenSource, HttpClient, SpClient};
use systemprompt_identifiers::DriveDeviceId;

/// The account a sync runs for and the folder it mirrors into.
pub struct SyncAccount {
    pub instance: SpClient,
    pub http: HttpClient,
    pub credentials: Arc<dyn DeviceTokenSource>,
    pub root: PathBuf,
    pub device_id: DriveDeviceId,
    pub host_name: Option<String>,
}

impl fmt::Debug for SyncAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncAccount")
            .field("instance", &self.instance.base())
            .field("root", &self.root)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

/// Timings of the sync loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOptions {
    pub poll_interval: Duration,
    pub debounce: Duration,
    pub disabled_recheck: Duration,
    pub recent_items: usize,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(20),
            debounce: Duration::from_millis(1500),
            disabled_recheck: Duration::from_secs(3600),
            recent_items: 50,
        }
    }
}
