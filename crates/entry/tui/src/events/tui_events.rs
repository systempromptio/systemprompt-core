use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, UserId};

use crate::messages::LogLevel;
use crate::state::ServiceStatus;

#[derive(Debug, Clone)]
pub enum TuiEvent {
    LogCreated(LogEventData),

    ServiceStatusChanged {
        service_name: String,
        status: ServiceStatus,
    },
    ServicesReconciled(Vec<ServiceStatus>),

    UserChanged {
        user_id: UserId,
    },
    SessionChanged {
        user_id: UserId,
        session_id: SessionId,
    },

    AnalyticsUpdated,
}

#[derive(Debug, Clone)]
pub struct LogEventData {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
}

impl LogEventData {
    pub fn new(
        timestamp: DateTime<Utc>,
        level: LogLevel,
        module: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp,
            level,
            module: module.into(),
            message: message.into(),
        }
    }
}
