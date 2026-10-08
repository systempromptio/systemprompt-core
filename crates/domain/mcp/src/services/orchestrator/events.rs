//! MCP orchestration event definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ServiceName;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpEvent {
    ServiceStartRequested {
        service_name: ServiceName,
    },
    ServiceStartCompleted {
        service_name: ServiceName,
        success: bool,
        pid: Option<u32>,
        port: Option<u16>,
        error: Option<String>,
        duration_ms: u64,
    },
    ServiceStarted {
        service_name: ServiceName,
        process_id: Option<u32>,
        port: u16,
    },
    ServiceFailed {
        service_name: ServiceName,
        error: String,
    },
    ServiceStopped {
        service_name: ServiceName,
        exit_code: Option<i32>,
    },
    SchemaUpdated {
        service_name: ServiceName,
        tool_count: usize,
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
    pub const fn service_name(&self) -> Option<&ServiceName> {
        match self {
            Self::ServiceStartRequested { service_name }
            | Self::ServiceStartCompleted { service_name, .. }
            | Self::ServiceStarted { service_name, .. }
            | Self::ServiceFailed { service_name, .. }
            | Self::ServiceStopped { service_name, .. }
            | Self::SchemaUpdated { service_name, .. } => Some(service_name),
            Self::ReconciliationStarted { .. } | Self::ReconciliationCompleted { .. } => None,
        }
    }

    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::ServiceStartRequested { .. } => "service_start_requested",
            Self::ServiceStartCompleted { .. } => "service_start_completed",
            Self::ServiceStarted { .. } => "service_started",
            Self::ServiceFailed { .. } => "service_failed",
            Self::ServiceStopped { .. } => "service_stopped",
            Self::SchemaUpdated { .. } => "schema_updated",
            Self::ReconciliationStarted { .. } => "reconciliation_started",
            Self::ReconciliationCompleted { .. } => "reconciliation_completed",
        }
    }

    pub const fn start_completed_success(
        name: ServiceName,
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

    pub const fn start_completed_failure(
        name: ServiceName,
        error: String,
        duration_ms: u64,
    ) -> Self {
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
