use serde::{Deserialize, Serialize};
use std::fmt;

pub use systemprompt_models::{RuntimeStatus, ServiceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesiredStatus {
    Enabled,
    Disabled,
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

impl fmt::Display for ServiceAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Start => write!(f, "start"),
            Self::Stop => write!(f, "stop"),
            Self::Restart => write!(f, "restart"),
            Self::CleanupDb => write!(f, "cleanup-db"),
            Self::CleanupProcess => write!(f, "cleanup-process"),
        }
    }
}
