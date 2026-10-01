//! The agent vocabulary for the `services.status` column.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }

    pub fn parse(stored: &str) -> Result<Self, RepositoryError> {
        match stored {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopped" => Ok(Self::Stopped),
            "error" => Ok(Self::Error),
            other => Err(RepositoryError::InvalidData(format!(
                "unrecognised agent service status '{other}'"
            ))),
        }
    }
}
