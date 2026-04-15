use serde::{Deserialize, Serialize};
use systemprompt_identifiers::AgentId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    AgentStartRequested {
        agent_id: AgentId,
    },
    AgentStartCompleted {
        agent_id: AgentId,
        success: bool,
        pid: Option<u32>,
        port: Option<u16>,
        error: Option<String>,
    },
    AgentStarted {
        agent_id: AgentId,
        pid: u32,
        port: u16,
    },
    AgentFailed {
        agent_id: AgentId,
        error: String,
    },
    AgentStopped {
        agent_id: AgentId,
        exit_code: Option<i32>,
    },
    AgentDisabled {
        agent_id: AgentId,
    },
    HealthCheckFailed {
        agent_id: AgentId,
        reason: String,
    },
    AgentRestartRequested {
        agent_id: AgentId,
        reason: String,
    },
    ReconciliationStarted {
        agent_count: usize,
    },
    ReconciliationCompleted {
        started: usize,
        failed: usize,
    },
}

impl AgentEvent {
    pub const fn agent_id(&self) -> Option<&AgentId> {
        match self {
            Self::AgentStartRequested { agent_id }
            | Self::AgentStartCompleted { agent_id, .. }
            | Self::AgentStarted { agent_id, .. }
            | Self::AgentFailed { agent_id, .. }
            | Self::AgentStopped { agent_id, .. }
            | Self::AgentDisabled { agent_id }
            | Self::HealthCheckFailed { agent_id, .. }
            | Self::AgentRestartRequested { agent_id, .. } => Some(agent_id),
            Self::ReconciliationStarted { .. } | Self::ReconciliationCompleted { .. } => None,
        }
    }

    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::AgentStartRequested { .. } => "agent_start_requested",
            Self::AgentStartCompleted { .. } => "agent_start_completed",
            Self::AgentStarted { .. } => "agent_started",
            Self::AgentFailed { .. } => "agent_failed",
            Self::AgentStopped { .. } => "agent_stopped",
            Self::AgentDisabled { .. } => "agent_disabled",
            Self::HealthCheckFailed { .. } => "health_check_failed",
            Self::AgentRestartRequested { .. } => "agent_restart_requested",
            Self::ReconciliationStarted { .. } => "reconciliation_started",
            Self::ReconciliationCompleted { .. } => "reconciliation_completed",
        }
    }
}
