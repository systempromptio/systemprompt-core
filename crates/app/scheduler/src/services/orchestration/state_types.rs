//! Lightweight types describing the desired and observed state of a service,
//! plus the actions the reconciler can take on them.
//!
//! [`VerifiedService`] pairs the persisted [`ServiceStatus`] vocabulary with
//! an `orphaned` observation: a live process holds the service's port but no
//! record of this instance owns it. The observation is never persisted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub use systemprompt_manifest::{ServiceStatus, ServiceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesiredStatus {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedService {
    pub status: ServiceStatus,
    pub orphaned: bool,
}

impl VerifiedService {
    #[must_use]
    pub const fn observed(status: ServiceStatus) -> Self {
        Self {
            status,
            orphaned: false,
        }
    }

    #[must_use]
    pub const fn orphaned() -> Self {
        Self {
            status: ServiceStatus::Running,
            orphaned: true,
        }
    }

    #[must_use]
    pub const fn is_healthy(self) -> bool {
        !self.orphaned
            && matches!(
                self.status,
                ServiceStatus::Running | ServiceStatus::Starting
            )
    }

    #[must_use]
    pub const fn needs_cleanup(self) -> bool {
        self.orphaned || matches!(self.status, ServiceStatus::Error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAction {
    None,
    Start,
    Stop,
    Restart,
    CleanupDb,
    CleanupProcess,
}

impl ServiceAction {
    pub const fn requires_process_change(&self) -> bool {
        matches!(
            self,
            Self::Start | Self::Stop | Self::Restart | Self::CleanupProcess
        )
    }

    pub const fn requires_db_change(&self) -> bool {
        matches!(
            self,
            Self::Start | Self::Stop | Self::Restart | Self::CleanupDb
        )
    }
}
