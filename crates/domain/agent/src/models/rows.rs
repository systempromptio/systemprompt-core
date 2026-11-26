//! Private decode targets of the report queries: `query_as!` converts each
//! column with `From<inferred type>`, which the validating identifier types
//! do not implement, so these rows decode identity columns as plain strings
//! and become typed rows through the trusted `new` constructor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AgentName, ContextId};
use systemprompt_models::a2a::TaskState;
use systemprompt_traits::usage_reports::agent::{
    AgentListRow, AgentTaskSample, ToolAgentUsageRow, ToolCaller,
};

pub(crate) struct TaskSampleDbRow {
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) status: Option<TaskState>,
    pub(crate) execution_time_ms: Option<i32>,
}

impl From<TaskSampleDbRow> for AgentTaskSample {
    fn from(row: TaskSampleDbRow) -> Self {
        Self {
            started_at: row.started_at,
            completed: row.status == Some(TaskState::Completed),
            execution_time_ms: row.execution_time_ms,
        }
    }
}

pub(crate) struct ToolCallerDbRow {
    pub(crate) agent_name: Option<String>,
    pub(crate) usage_count: i64,
}

impl From<ToolCallerDbRow> for ToolAgentUsageRow {
    fn from(row: ToolCallerDbRow) -> Self {
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

pub(crate) struct AgentListDbRow {
    pub(crate) agent_name: String,
    pub(crate) task_count: i64,
    pub(crate) completed_count: i64,
    pub(crate) avg_execution_time_ms: i64,
    pub(crate) total_cost_microdollars: i64,
    pub(crate) last_active: DateTime<Utc>,
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

pub(crate) struct RecentContextDbRow {
    pub(crate) context_id: ContextId,
    pub(crate) last_activity: DateTime<Utc>,
    pub(crate) ai_requests: i64,
    pub(crate) model: Option<String>,
    pub(crate) agent_name: Option<String>,
}
