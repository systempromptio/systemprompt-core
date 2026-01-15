use serde::{Deserialize, Serialize};

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
