//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so these rows decode identity columns
//! (`user_id`, `agent_name`, `tool_name`, `server_name`) as plain strings and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AgentName, ContextId, McpServerId, McpToolName};

use super::reporting::{
    AgentListRow, RecentContextRow, ToolAgentUsageRow, ToolCaller, ToolListRow,
};

#[derive(Debug)]
pub(crate) struct AgentListDbRow {
    pub agent_name: String,
    pub task_count: i64,
    pub completed_count: i64,
    pub avg_execution_time_ms: i64,
    pub total_cost_microdollars: i64,
    pub last_active: DateTime<Utc>,
}

impl From<AgentListDbRow> for AgentListRow {
    fn from(row: AgentListDbRow) -> Self {
        Self {
            agent_name: AgentName::new(row.agent_name),
            task_count: row.task_count,
            completed_count: row.completed_count,
            avg_execution_time_ms: row.avg_execution_time_ms,
            total_cost_microdollars: row.total_cost_microdollars,
            last_active: row.last_active,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ToolListDbRow {
    pub tool_name: String,
    pub server_name: String,
    pub execution_count: i64,
    pub success_count: i64,
    pub avg_time: f64,
    pub last_used: DateTime<Utc>,
}

impl From<ToolListDbRow> for ToolListRow {
    fn from(row: ToolListDbRow) -> Self {
        Self {
            tool_name: McpToolName::new(row.tool_name),
            server_name: McpServerId::new(row.server_name),
            execution_count: row.execution_count,
            success_count: row.success_count,
            avg_time: row.avg_time,
            last_used: row.last_used,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ToolAgentUsageDbRow {
    pub agent_name: Option<String>,
    pub usage_count: i64,
}

impl From<ToolAgentUsageDbRow> for ToolAgentUsageRow {
    fn from(row: ToolAgentUsageDbRow) -> Self {
        let caller = match row.agent_name.as_deref() {
            None | Some(ToolCaller::DIRECT_CALL_LABEL) => ToolCaller::DirectCall,
            Some(ToolCaller::UNLINKED_TASK_LABEL) => ToolCaller::UnlinkedTask,
            Some(name) => ToolCaller::Agent(AgentName::new(name)),
        };
        Self {
            caller,
            usage_count: row.usage_count,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RecentContextDbRow {
    pub context_id: ContextId,
    pub last_activity: DateTime<Utc>,
    pub ai_requests: i64,
    pub model: Option<String>,
    pub agent_name: Option<String>,
    pub context_name: Option<String>,
}

impl From<RecentContextDbRow> for RecentContextRow {
    fn from(row: RecentContextDbRow) -> Self {
        Self {
            context_id: row.context_id,
            last_activity: row.last_activity,
            ai_requests: row.ai_requests,
            model: row.model,
            agent_name: row.agent_name.map(AgentName::new),
            context_name: row.context_name,
        }
    }
}
