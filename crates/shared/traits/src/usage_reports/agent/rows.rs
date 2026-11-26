//! Row types, sort order and tool-caller label the agent activity report
//! port returns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, ContextId, UserId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSortOrder {
    #[default]
    TaskCount,
    SuccessRate,
    Cost,
    LastActive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListRow {
    pub agent_name: AgentName,
    pub task_count: i64,
    pub completed_count: i64,
    pub avg_execution_time_ms: i64,
    pub total_cost_microdollars: i64,
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentStatsRow {
    pub total_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub failed_tasks: i64,
    pub avg_execution_time_ms: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentAiStatsRow {
    pub total_ai_requests: i64,
    pub total_cost_microdollars: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentTaskSample {
    pub started_at: DateTime<Utc>,
    pub completed: bool,
    pub execution_time_ms: Option<i32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentSummaryRow {
    pub total_tasks: i64,
    pub completed: i64,
    pub failed: i64,
    pub avg_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusBreakdownRow {
    pub status: String,
    pub status_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentErrorRow {
    pub error_type: Option<String>,
    pub error_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentHourlyRow {
    pub task_hour: i32,
    pub task_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AgentOverviewRow {
    pub active_agents: i64,
    pub total_tasks: i64,
    pub completed_tasks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContextRow {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: Option<String>,
    pub task_count: i64,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TaskStatsRow {
    pub task_count: i64,
    pub avg_execution_time_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentContextRow {
    pub context_id: ContextId,
    pub last_activity: DateTime<Utc>,
    pub ai_requests: i64,
    pub model: Option<String>,
    pub agent_name: Option<AgentName>,
    pub context_name: Option<String>,
}

/// Who made a tool call: an agent's task, or a call no agent is recorded
/// for (outside any task, or in a task whose agent is not recorded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
pub enum ToolCaller {
    Agent(AgentName),
    Unattributed,
}

impl ToolCaller {
    pub const UNATTRIBUTED_LABEL: &'static str = "Unattributed";

    pub fn as_str(&self) -> &str {
        match self {
            Self::Agent(name) => name.as_str(),
            Self::Unattributed => Self::UNATTRIBUTED_LABEL,
        }
    }
}

impl std::fmt::Display for ToolCaller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ToolCaller> for String {
    fn from(caller: ToolCaller) -> Self {
        caller.as_str().to_owned()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolAgentUsageRow {
    pub caller: ToolCaller,
    pub usage_count: i64,
}
