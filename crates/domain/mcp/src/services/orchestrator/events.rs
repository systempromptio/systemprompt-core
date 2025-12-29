use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpEvent {
    ServiceStartRequested {
        service_name: String,
    },
    ServiceStartCompleted {
        service_name: String,
        success: bool,
        pid: Option<u32>,
        port: Option<u16>,
        error: Option<String>,
        duration_ms: u64,
    },
    ServiceStarted {
        service_name: String,
        process_id: u32,
        port: u16,
    },
    ServiceFailed {
        service_name: String,
        error: String,
    },
    ServiceStopped {
        service_name: String,
        exit_code: Option<i32>,
    },
    HealthCheckFailed {
        service_name: String,
        reason: String,
    },
    SchemaUpdated {
        service_name: String,
        tool_count: usize,
    },
    ServiceRestartRequested {
        service_name: String,
        reason: String,
    },
    ReconciliationStarted {
        service_count: usize,
    },
    ReconciliationCompleted {
        started: usize,
        failed: usize,
        duration_ms: u64,
    },
}

impl McpEvent {
    pub fn service_name(&self) -> &str {
        match self {
            Self::ServiceStartRequested { service_name }
            | Self::ServiceStartCompleted { service_name, .. }
            | Self::ServiceStarted { service_name, .. }
            | Self::ServiceFailed { service_name, .. }
            | Self::ServiceStopped { service_name, .. }
            | Self::HealthCheckFailed { service_name, .. }
            | Self::SchemaUpdated { service_name, .. }
            | Self::ServiceRestartRequested { service_name, .. } => service_name,
            Self::ReconciliationStarted { .. } | Self::ReconciliationCompleted { .. } => "",
        }
    }

    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::ServiceStartRequested { .. } => "service_start_requested",
            Self::ServiceStartCompleted { .. } => "service_start_completed",
            Self::ServiceStarted { .. } => "service_started",
            Self::ServiceFailed { .. } => "service_failed",
            Self::ServiceStopped { .. } => "service_stopped",
            Self::HealthCheckFailed { .. } => "health_check_failed",
            Self::SchemaUpdated { .. } => "schema_updated",
            Self::ServiceRestartRequested { .. } => "service_restart_requested",
            Self::ReconciliationStarted { .. } => "reconciliation_started",
            Self::ReconciliationCompleted { .. } => "reconciliation_completed",
        }
    }

    pub const fn start_completed_success(
        name: String,
        pid: u32,
        port: u16,
        duration_ms: u64,
    ) -> Self {
        Self::ServiceStartCompleted {
            service_name: name,
            success: true,
            pid: Some(pid),
            port: Some(port),
            error: None,
            duration_ms,
        }
    }

    pub const fn start_completed_failure(name: String, error: String, duration_ms: u64) -> Self {
        Self::ServiceStartCompleted {
            service_name: name,
            success: false,
            pid: None,
            port: None,
            error: Some(error),
            duration_ms,
        }
    }
}
