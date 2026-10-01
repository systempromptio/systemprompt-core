//! The agent vocabulary for the `services.status` column.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::ServiceStatus;
use systemprompt_traits::RepositoryError;

/// Lifecycle state of an agent row in the shared `services` table; the only
/// values the agent module writes and the only ones it accepts on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentServiceStatus {
    Starting,
    Running,
    Stopped,
    Error,
}

impl AgentServiceStatus {
    pub const fn as_str(self) -> &'static str {
        self.service_status().as_str()
    }

    pub const fn service_status(self) -> ServiceStatus {
        match self {
            Self::Starting => ServiceStatus::Starting,
            Self::Running => ServiceStatus::Running,
            Self::Stopped => ServiceStatus::Stopped,
            Self::Error => ServiceStatus::Error,
        }
    }

    pub fn from_service_status(stored: ServiceStatus) -> Result<Self, RepositoryError> {
        match stored {
            ServiceStatus::Starting => Ok(Self::Starting),
            ServiceStatus::Running => Ok(Self::Running),
            ServiceStatus::Stopped => Ok(Self::Stopped),
            ServiceStatus::Error => Ok(Self::Error),
            other @ ServiceStatus::Stopping => Err(RepositoryError::InvalidData(format!(
                "unrecognised agent service status '{other}'"
            ))),
        }
    }
}
