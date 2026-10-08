//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so these rows decode identity columns
//! (`user_id`, `agent_name`, `tool_name`, `server_name`) as plain strings and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AgentName, ContentId, ContextId, EngagementEventId, McpServerId, McpToolName, SessionId, UserId,
};

use super::EngagementEvent;
use super::reporting::{
    AgentListRow, RecentContextRow, ToolAgentUsageRow, ToolCaller, ToolListRow,
};

#[derive(Debug)]
pub(crate) struct EngagementEventRow {
    pub id: EngagementEventId,
    pub session_id: SessionId,
    pub user_id: String,
    pub page_url: String,
    pub content_id: Option<ContentId>,
    pub event_type: String,
    pub time_on_page_ms: i32,
    pub time_to_first_interaction_ms: Option<i32>,
    pub time_to_first_scroll_ms: Option<i32>,
    pub max_scroll_depth: i32,
    pub scroll_velocity_avg: Option<f32>,
    pub scroll_direction_changes: Option<i32>,
    pub click_count: i32,
    pub mouse_move_distance_px: Option<i32>,
    pub keyboard_events: Option<i32>,
    pub copy_events: Option<i32>,
    pub focus_time_ms: i32,
    pub blur_count: i32,
    pub tab_switches: i32,
    pub visible_time_ms: i32,
    pub hidden_time_ms: i32,
    pub is_rage_click: Option<bool>,
    pub is_dead_click: Option<bool>,
    pub reading_pattern: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<EngagementEventRow> for EngagementEvent {
    fn from(row: EngagementEventRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            user_id: UserId::new(row.user_id),
            page_url: row.page_url,
            content_id: row.content_id,
            event_type: row.event_type,
            time_on_page_ms: row.time_on_page_ms,
            time_to_first_interaction_ms: row.time_to_first_interaction_ms,
            time_to_first_scroll_ms: row.time_to_first_scroll_ms,
            max_scroll_depth: row.max_scroll_depth,
            scroll_velocity_avg: row.scroll_velocity_avg,
            scroll_direction_changes: row.scroll_direction_changes,
            click_count: row.click_count,
            mouse_move_distance_px: row.mouse_move_distance_px,
            keyboard_events: row.keyboard_events,
            copy_events: row.copy_events,
            focus_time_ms: row.focus_time_ms,
            blur_count: row.blur_count,
            tab_switches: row.tab_switches,
            visible_time_ms: row.visible_time_ms,
            hidden_time_ms: row.hidden_time_ms,
            is_rage_click: row.is_rage_click,
            is_dead_click: row.is_dead_click,
            reading_pattern: row.reading_pattern,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

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
