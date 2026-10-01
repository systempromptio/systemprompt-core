//! CLI-facing tool-usage analytics rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{AgentName, McpServerId, McpToolName};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolListRow {
    pub tool_name: McpToolName,
    pub server_name: McpServerId,
    pub execution_count: i64,
    pub success_count: i64,
    pub avg_time: f64,
    pub last_used: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ToolStatsRow {
    pub total_tools: i64,
    pub total_executions: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromRow)]
pub struct ToolSummaryRow {
    pub total: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolStatusBreakdownRow {
    pub status: String,
    pub status_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolErrorRow {
    pub error_msg: Option<String>,
    pub error_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
pub enum ToolCaller {
    Agent(AgentName),
    DirectCall,
    UnlinkedTask,
}

impl ToolCaller {
    pub const DIRECT_CALL_LABEL: &'static str = "Direct Call";
    pub const UNLINKED_TASK_LABEL: &'static str = "Unlinked Task";

    pub fn as_str(&self) -> &str {
        match self {
            Self::Agent(name) => name.as_str(),
            Self::DirectCall => Self::DIRECT_CALL_LABEL,
            Self::UnlinkedTask => Self::UNLINKED_TASK_LABEL,
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolExecutionRow {
    pub created_at: DateTime<Utc>,
    pub status: Option<String>,
    pub execution_time_ms: Option<i32>,
}
