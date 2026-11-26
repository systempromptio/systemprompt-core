//! The agent member's implementation of the agent activity report port.
//!
//! [`AgentActivityReport`] reads the member's report views and the views it
//! declares over the ai and mcp members' report views
//! (`report_agent_ai_usage_hourly`, `report_agent_ai_context_hourly`,
//! `report_agent_tool_executions`).
//!
//! Per-agent task listings aggregate the hourly task rollup and the hourly AI
//! usage rollup per agent before joining them, both widened to whole UTC
//! hours at the window's start, so a task is never repeated once per request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agent_detail;
mod agents;
mod conversations;
mod listing;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::agent::{
    AgentActivityReport, AgentAiStatsRow, AgentContextRow, AgentErrorRow, AgentHourlyRow,
    AgentListRow, AgentOverviewRow, AgentSortOrder, AgentStatsRow, AgentStatusBreakdownRow,
    AgentSummaryRow, AgentTaskSample, RecentContextRow, TaskStatsRow, ToolAgentUsageRow,
};

#[derive(Debug, Clone)]
pub struct AgentActivityReportRepository {
    pool: Arc<PgPool>,
}

impl AgentActivityReportRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }
}

fn contains_pattern(filter: &str) -> String {
    format!("%{filter}%")
}

#[async_trait]
impl AgentActivityReport for AgentActivityReportRepository {
    async fn list_agents(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort: AgentSortOrder,
    ) -> Result<Vec<AgentListRow>, RepositoryError> {
        self.list_agents_sorted(start, end, limit, sort).await
    }

    async fn agent_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<AgentStatsRow, RepositoryError> {
        self.stats(start, end, agent_filter).await
    }

    async fn agent_ai_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentAiStatsRow, RepositoryError> {
        self.ai_stats(start, end).await
    }

    async fn task_samples(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<Vec<AgentTaskSample>, RepositoryError> {
        self.samples(start, end, agent_filter).await
    }

    async fn agent_task_count(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        self.task_count_for(agent_filter, start, end).await
    }

    async fn agent_summary(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentSummaryRow, RepositoryError> {
        self.summary(agent_filter, start, end).await
    }

    async fn status_breakdown(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentStatusBreakdownRow>, RepositoryError> {
        self.statuses(agent_filter, start, end).await
    }

    async fn top_errors(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentErrorRow>, RepositoryError> {
        self.errors(agent_filter, start, end).await
    }

    async fn hourly_distribution(
        &self,
        agent_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentHourlyRow>, RepositoryError> {
        self.hours(agent_filter, start, end).await
    }

    async fn agent_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentOverviewRow, RepositoryError> {
        self.overview(start, end).await
    }

    async fn list_agent_contexts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<AgentContextRow>, RepositoryError> {
        self.contexts(start, end, limit, user).await
    }

    async fn agent_context_sessions(
        &self,
        session_ids: &[SessionId],
    ) -> Result<Vec<SessionId>, RepositoryError> {
        self.context_sessions(session_ids).await
    }

    async fn context_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        self.count_contexts(start, end).await
    }

    async fn task_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TaskStatsRow, RepositoryError> {
        self.tasks_in(start, end).await
    }

    async fn message_count(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        self.count_messages(start, end).await
    }

    async fn context_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        self.context_times(start, end).await
    }

    async fn task_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        self.task_times(start, end).await
    }

    async fn message_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        self.message_times(start, end).await
    }

    async fn recent_contexts_for_user(
        &self,
        user_id: &UserId,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RecentContextRow>, RepositoryError> {
        self.recent_contexts(user_id, end, limit).await
    }

    async fn tool_usage_by_agent(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolAgentUsageRow>, RepositoryError> {
        self.tool_callers(tool_filter, start, end).await
    }
}
