//! The tool-usage report port: the MCP tool leaderboard, aggregate and
//! per-tool statistics, status and error breakdowns, the execution series
//! the trend charts bucket, and the overview's tool totals.
//!
//! The `mcp` member implements it over its report views
//! (`report_mcp_tool_executions`, `report_tool_usage_hourly`) and provides
//! it open as [`DynToolUsageReport`]. Usage by calling agent joins the agent
//! member's tasks and is read from that member's report port instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpServerId, McpToolName};

use crate::repository::RepositoryError;

/// The order of the tool leaderboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToolListSort {
    #[default]
    ExecutionCount,
    SuccessRate,
    AvgTime,
}

/// The window, size, server filter and order of a tool leaderboard read.
#[derive(Debug, Clone, Copy)]
pub struct ToolListParams<'a> {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: i64,
    pub server_filter: Option<&'a str>,
    pub sort: ToolListSort,
}

/// One tool on the leaderboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListRow {
    pub tool_name: McpToolName,
    pub server_name: McpServerId,
    pub execution_count: i64,
    pub success_count: i64,
    pub avg_time: f64,
    pub last_used: DateTime<Utc>,
}

/// Aggregate execution statistics over every tool in a window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ToolStatsRow {
    pub total_tools: i64,
    pub total_executions: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

/// Execution statistics of the tools a name filter matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ToolSummaryRow {
    pub total: i64,
    pub successful: i64,
    pub failed: i64,
    pub timeout: i64,
    pub avg_time: f64,
    pub p95_time: f64,
}

/// How many executions ended in one status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatusBreakdownRow {
    pub status: String,
    pub status_count: i64,
}

/// One failure message prefix and how often it occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorRow {
    pub error_msg: Option<String>,
    pub error_count: i64,
}

/// One execution of the trend series: when, its recorded status, and its
/// duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRow {
    pub created_at: DateTime<Utc>,
    pub status: Option<String>,
    pub execution_time_ms: Option<i32>,
}

/// The overview's tool totals: executions and successful executions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ToolOverviewMetrics {
    pub total: i64,
    pub successful: i64,
}

/// Reads the tool-usage reports.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// `DynToolUsageReport`.
#[async_trait]
pub trait ToolUsageReport: Send + Sync {
    async fn list_tools(
        &self,
        params: ToolListParams<'_>,
    ) -> Result<Vec<ToolListRow>, RepositoryError>;

    async fn stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<ToolStatsRow, RepositoryError>;

    async fn tool_execution_count(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError>;

    async fn tool_summary(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolSummaryRow, RepositoryError>;

    async fn status_breakdown(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolStatusBreakdownRow>, RepositoryError>;

    async fn top_errors(
        &self,
        tool_filter: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ToolErrorRow>, RepositoryError>;

    async fn executions_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        tool_filter: Option<&str>,
    ) -> Result<Vec<ToolExecutionRow>, RepositoryError>;

    async fn overview_tool_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ToolOverviewMetrics, RepositoryError>;
}

pub type DynToolUsageReport = Arc<dyn ToolUsageReport>;
