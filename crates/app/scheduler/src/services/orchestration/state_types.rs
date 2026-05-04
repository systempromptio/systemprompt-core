//! Lightweight enums describing desired and runtime status of a service,
//! plus the actions the reconciler can take on them.

use serde::{Deserialize, Serialize};

pub use systemprompt_models::{RuntimeStatus, ServiceType};

/// Whether a service should be running per its configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesiredStatus {
    /// Service is configured to run.
    Enabled,
    /// Service is configured off.
    Disabled,
}

/// Action the reconciler must take to converge runtime to desired state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAction {
    /// State already matches; no action required.
    None,
    /// Start the service binary on its configured port.
    Start,
    /// Stop the service gracefully and clear its DB record.
    Stop,
    /// Stop and immediately restart the service.
    Restart,
    /// Delete a stale DB row whose process is already gone.
    CleanupDb,
    /// Reap an orphan process that has no DB row referencing it.
    CleanupProcess,
}

impl ServiceAction {
    /// Return whether this action requires the OS process state to change.
    pub const fn requires_process_change(&self) -> bool {
        matches!(
            self,
            Self::Start | Self::Stop | Self::Restart | Self::CleanupProcess
        )
    }

    /// Return whether this action requires a write to the `services` table.
    pub const fn requires_db_change(&self) -> bool {
        matches!(
            self,
            Self::Start | Self::Stop | Self::Restart | Self::CleanupDb
        )
    }
}
