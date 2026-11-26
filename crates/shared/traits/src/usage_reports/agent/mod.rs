//! The agent activity report: per-agent task metrics, agent conversation
//! contexts and their trends, the tool calls each agent made, and the recent
//! contexts of one user.
//!
//! The `agent` member implements [`AgentActivityReport`] over its report
//! views (and the views it declares over the `ai` and `mcp` members' report
//! views it requires) and provides it as [`DynAgentActivityReport`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rows;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{SessionId, UserId};

use crate::RepositoryError;

pub use rows::{
    AgentAiStatsRow, AgentContextRow, AgentErrorRow, AgentHourlyRow, AgentListRow,
    AgentOverviewRow, AgentSortOrder, AgentStatsRow, AgentStatusBreakdownRow, AgentSummaryRow,
    AgentTaskSample, RecentContextRow, TaskStatsRow, ToolAgentUsageRow, ToolCaller,
};

/// Agent activity over a `[start, end)` window; an agent filter matches any
/// agent whose name contains it, case-insensitively.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// [`DynAgentActivityReport`].
#[async_trait]
pub trait AgentActivityReport: Send + Sync {
    async fn list_agents(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort: AgentSortOrder,
    ) -> Result<Vec<AgentListRow>, RepositoryError>;

    async fn agent_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<AgentStatsRow, RepositoryError>;

    async fn agent_ai_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentAiStatsRow, RepositoryError>;

    async fn task_samples(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<Vec<AgentTaskSample>, RepositoryError>;

    async fn agent_task_count(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError>;

    async fn agent_summary(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentSummaryRow, RepositoryError>;

    async fn status_breakdown(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentStatusBreakdownRow>, RepositoryError>;

    async fn top_errors(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentErrorRow>, RepositoryError>;

    async fn hourly_distribution(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentHourlyRow>, RepositoryError>;

    async fn agent_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentOverviewRow, RepositoryError>;

    async fn list_agent_contexts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<AgentContextRow>, RepositoryError>;

    async fn agent_context_sessions(
        &self,
        session_ids: &[SessionId],
    ) -> Result<Vec<SessionId>, RepositoryError>;

    async fn context_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError>;

    async fn task_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TaskStatsRow, RepositoryError>;

    async fn message_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError>;

    async fn context_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError>;

    async fn task_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError>;

    async fn message_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError>;

    async fn recent_contexts_for_user(
        &self,
        user_id: &UserId,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RecentContextRow>, RepositoryError>;

    async fn tool_usage_by_agent(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolAgentUsageRow>, RepositoryError>;
}

pub type DynAgentActivityReport = Arc<dyn AgentActivityReport>;
