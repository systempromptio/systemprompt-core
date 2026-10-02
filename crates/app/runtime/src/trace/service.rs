//! Read-side query facade over the tracing and audit tables.
//!
//! [`TraceQueryService`] is the single entry point for reconstructing a trace
//! from its constituent rows (logs, AI requests, MCP tool executions, task
//! execution steps) and for the log/audit browsing surfaces. Each public method
//! reads through the injected [`TraceRepository`];
//! [`get_all_trace_data`] fans the per-source fetches out concurrently.
//!
//! [`get_all_trace_data`]: TraceQueryService::get_all_trace_data
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AiRequestId, LogId, TaskId, TraceId};

use systemprompt_logging::models::{LogEntry, LogLevel};

use super::TraceError;

pub(super) type Result<T> = std::result::Result<T, TraceError>;

use super::models::{
    AiRequestDetail, AiRequestFilter, AiRequestListItem, AiRequestStats, AiRequestSummary,
    AuditLookupResult, AuditPage, AuditToolCallRow, ConversationMessage, ExecutionStepSummary,
    LevelCount, LinkedMcpCall, LogSearchItem, LogTimeRange, McpExecutionSummary, ModuleCount,
    ToolExecutionFilter, ToolExecutionItem, TraceEvent, TraceListFilter, TraceListItem,
};
use super::repository::TraceRepository;

#[derive(Debug, Clone)]
pub struct TraceQueryService {
    repository: TraceRepository,
}

impl TraceQueryService {
    pub const fn new(repository: TraceRepository) -> Self {
        Self { repository }
    }

    pub async fn get_log_events(&self, trace_id: &TraceId) -> Result<Vec<TraceEvent>> {
        self.repository.fetch_log_events(trace_id).await
    }

    pub async fn get_ai_request_summary(&self, trace_id: &TraceId) -> Result<AiRequestSummary> {
        self.repository.fetch_ai_request_summary(trace_id).await
    }

    pub async fn get_ai_request_events(&self, trace_id: &TraceId) -> Result<Vec<TraceEvent>> {
        self.repository.fetch_ai_request_events(trace_id).await
    }

    pub async fn get_mcp_execution_summary(
        &self,
        trace_id: &TraceId,
    ) -> Result<McpExecutionSummary> {
        self.repository.fetch_mcp_execution_summary(trace_id).await
    }

    pub async fn get_mcp_execution_events(&self, trace_id: &TraceId) -> Result<Vec<TraceEvent>> {
        self.repository.fetch_mcp_execution_events(trace_id).await
    }

    pub async fn get_task_id(&self, trace_id: &TraceId) -> Result<Option<TaskId>> {
        Ok(self
            .repository
            .fetch_task_id_for_trace(trace_id)
            .await?
            .map(TaskId::new))
    }

    pub async fn get_execution_step_summary(
        &self,
        trace_id: &TraceId,
    ) -> Result<ExecutionStepSummary> {
        self.repository.fetch_execution_step_summary(trace_id).await
    }

    pub async fn get_execution_step_events(&self, trace_id: &TraceId) -> Result<Vec<TraceEvent>> {
        self.repository.fetch_execution_step_events(trace_id).await
    }

    pub async fn get_all_trace_data(
        &self,
        trace_id: &TraceId,
    ) -> Result<(
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        AiRequestSummary,
        McpExecutionSummary,
        ExecutionStepSummary,
        Option<TaskId>,
    )> {
        tokio::try_join!(
            self.get_log_events(trace_id),
            self.get_ai_request_events(trace_id),
            self.get_mcp_execution_events(trace_id),
            self.get_execution_step_events(trace_id),
            self.get_ai_request_summary(trace_id),
            self.get_mcp_execution_summary(trace_id),
            self.get_execution_step_summary(trace_id),
            self.get_task_id(trace_id),
        )
    }

    pub async fn list_traces(&self, filter: &TraceListFilter) -> Result<Vec<TraceListItem>> {
        self.repository.list_traces(filter).await
    }

    pub async fn list_tool_executions(
        &self,
        filter: &ToolExecutionFilter,
    ) -> Result<Vec<ToolExecutionItem>> {
        self.repository.list_tool_executions(filter).await
    }

    pub async fn search_logs(
        &self,
        pattern: &str,
        since: Option<DateTime<Utc>>,
        level: Option<LogLevel>,
        limit: i64,
    ) -> Result<Vec<LogSearchItem>> {
        self.repository
            .search_logs(pattern, since, level, limit)
            .await
    }

    pub async fn search_tool_executions(
        &self,
        pattern: &str,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<ToolExecutionItem>> {
        self.repository
            .search_tool_executions(pattern, since, limit)
            .await
    }

    pub async fn list_ai_requests(
        &self,
        filter: &AiRequestFilter,
    ) -> Result<Vec<AiRequestListItem>> {
        self.repository.list_ai_requests(filter).await
    }

    pub async fn get_ai_request_stats(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<AiRequestStats> {
        self.repository.get_ai_request_stats(since).await
    }

    pub async fn find_ai_request_detail(&self, id: &str) -> Result<Option<AiRequestDetail>> {
        self.repository.find_ai_request_detail(id).await
    }

    pub async fn find_ai_request_for_audit(&self, id: &str) -> Result<Option<AuditLookupResult>> {
        self.repository.find_ai_request_for_audit(id).await
    }

    pub async fn count_audit_messages(&self, request_id: &AiRequestId) -> Result<i64> {
        self.repository.count_audit_messages(request_id).await
    }

    pub async fn count_audit_tool_calls(&self, request_id: &AiRequestId) -> Result<i64> {
        self.repository.count_audit_tool_calls(request_id).await
    }

    pub async fn list_audit_messages(
        &self,
        request_id: &AiRequestId,
        page: AuditPage,
    ) -> Result<Vec<ConversationMessage>> {
        self.repository.list_audit_messages(request_id, page).await
    }

    pub async fn list_audit_tool_calls(
        &self,
        request_id: &AiRequestId,
        page: AuditPage,
    ) -> Result<Vec<AuditToolCallRow>> {
        self.repository
            .list_audit_tool_calls(request_id, page)
            .await
    }

    pub async fn list_linked_mcp_calls(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<LinkedMcpCall>> {
        self.repository.list_linked_mcp_calls(request_id).await
    }

    pub async fn find_log_by_id(&self, id: &LogId) -> Result<Option<LogEntry>> {
        self.repository.find_log_by_id(id).await
    }

    pub async fn find_log_by_partial_id(&self, id_prefix: &str) -> Result<Option<LogEntry>> {
        self.repository.find_log_by_partial_id(id_prefix).await
    }

    pub async fn find_logs_by_trace_id(&self, trace_id: &TraceId) -> Result<Vec<LogEntry>> {
        self.repository.find_logs_by_trace_id(trace_id).await
    }

    pub async fn list_logs_filtered(
        &self,
        since: Option<DateTime<Utc>>,
        level: Option<LogLevel>,
        limit: i64,
    ) -> Result<Vec<LogEntry>> {
        self.repository
            .list_logs_filtered(since, level, limit)
            .await
    }

    pub async fn count_logs_by_level(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<LevelCount>> {
        self.repository.count_logs_by_level(since).await
    }

    pub async fn top_modules(
        &self,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<ModuleCount>> {
        self.repository.top_modules(since, limit).await
    }

    pub async fn log_time_range(&self, since: Option<DateTime<Utc>>) -> Result<LogTimeRange> {
        self.repository.log_time_range(since).await
    }

    pub async fn total_log_count(&self) -> Result<i64> {
        self.repository.total_log_count().await
    }
}
