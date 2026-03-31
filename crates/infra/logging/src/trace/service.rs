use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::LogEntry;

use super::models::{
    AiRequestDetail, AiRequestListItem, AiRequestStats, AiRequestSummary, AuditLookupResult,
    AuditToolCallRow, ConversationMessage, ExecutionStepSummary, LevelCount, LinkedMcpCall,
    LogSearchItem, LogTimeRange, McpExecutionSummary, ModuleCount, ToolExecutionFilter,
    ToolExecutionItem, TraceEvent, TraceListFilter, TraceListItem,
};
use super::{
    audit_queries, list_queries, log_lookup_queries, log_search_queries, log_summary_queries,
    queries, request_queries, tool_queries,
};

#[derive(Debug, Clone)]
pub struct TraceQueryService {
    pool: Arc<PgPool>,
}

impl TraceQueryService {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn get_log_events(&self, trace_id: &str) -> Result<Vec<TraceEvent>> {
        queries::fetch_log_events(&self.pool, trace_id).await
    }

    pub async fn get_ai_request_summary(&self, trace_id: &str) -> Result<AiRequestSummary> {
        queries::fetch_ai_request_summary(&self.pool, trace_id).await
    }

    pub async fn get_ai_request_events(&self, trace_id: &str) -> Result<Vec<TraceEvent>> {
        queries::fetch_ai_request_events(&self.pool, trace_id).await
    }

    pub async fn get_mcp_execution_summary(&self, trace_id: &str) -> Result<McpExecutionSummary> {
        queries::fetch_mcp_execution_summary(&self.pool, trace_id).await
    }

    pub async fn get_mcp_execution_events(&self, trace_id: &str) -> Result<Vec<TraceEvent>> {
        queries::fetch_mcp_execution_events(&self.pool, trace_id).await
    }

    pub async fn get_task_id(&self, trace_id: &str) -> Result<Option<String>> {
        queries::fetch_task_id_for_trace(&self.pool, trace_id).await
    }

    pub async fn get_execution_step_summary(&self, trace_id: &str) -> Result<ExecutionStepSummary> {
        queries::fetch_execution_step_summary(&self.pool, trace_id).await
    }

    pub async fn get_execution_step_events(&self, trace_id: &str) -> Result<Vec<TraceEvent>> {
        queries::fetch_execution_step_events(&self.pool, trace_id).await
    }

    pub async fn get_all_trace_data(
        &self,
        trace_id: &str,
    ) -> Result<(
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        Vec<TraceEvent>,
        AiRequestSummary,
        McpExecutionSummary,
        ExecutionStepSummary,
        Option<String>,
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
        list_queries::list_traces(&self.pool, filter).await
    }

    pub async fn list_tool_executions(
        &self,
        filter: &ToolExecutionFilter,
    ) -> Result<Vec<ToolExecutionItem>> {
        tool_queries::list_tool_executions(&self.pool, filter).await
    }

    pub async fn search_logs(
        &self,
        pattern: &str,
        since: Option<DateTime<Utc>>,
        level: Option<&str>,
        limit: i64,
    ) -> Result<Vec<LogSearchItem>> {
        log_search_queries::search_logs(&self.pool, pattern, since, level, limit).await
    }

    pub async fn search_tool_executions(
        &self,
        pattern: &str,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<ToolExecutionItem>> {
        log_search_queries::search_tool_executions(&self.pool, pattern, since, limit).await
    }

    pub async fn list_ai_requests(
        &self,
        since: Option<DateTime<Utc>>,
        model: Option<&str>,
        provider: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AiRequestListItem>> {
        request_queries::list_ai_requests(&self.pool, since, model, provider, limit).await
    }

    pub async fn get_ai_request_stats(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<AiRequestStats> {
        request_queries::get_ai_request_stats(&self.pool, since).await
    }

    pub async fn find_ai_request_detail(&self, id: &str) -> Result<Option<AiRequestDetail>> {
        request_queries::find_ai_request_detail(&self.pool, id).await
    }

    pub async fn find_ai_request_for_audit(&self, id: &str) -> Result<Option<AuditLookupResult>> {
        audit_queries::find_ai_request_for_audit(&self.pool, id).await
    }

    pub async fn list_audit_messages(&self, request_id: &str) -> Result<Vec<ConversationMessage>> {
        audit_queries::list_audit_messages(&self.pool, request_id).await
    }

    pub async fn list_audit_tool_calls(&self, request_id: &str) -> Result<Vec<AuditToolCallRow>> {
        audit_queries::list_audit_tool_calls(&self.pool, request_id).await
    }

    pub async fn list_linked_mcp_calls(&self, request_id: &str) -> Result<Vec<LinkedMcpCall>> {
        audit_queries::list_linked_mcp_calls(&self.pool, request_id).await
    }

    pub async fn find_log_by_id(&self, id: &str) -> Result<Option<LogEntry>> {
        log_lookup_queries::find_log_by_id(&self.pool, id).await
    }

    pub async fn find_log_by_partial_id(&self, id_prefix: &str) -> Result<Option<LogEntry>> {
        log_lookup_queries::find_log_by_partial_id(&self.pool, id_prefix).await
    }

    pub async fn find_logs_by_trace_id(&self, trace_id: &str) -> Result<Vec<LogEntry>> {
        log_lookup_queries::find_logs_by_trace_id(&self.pool, trace_id).await
    }

    pub async fn list_logs_filtered(
        &self,
        since: Option<DateTime<Utc>>,
        level: Option<&str>,
        limit: i64,
    ) -> Result<Vec<LogEntry>> {
        log_lookup_queries::list_logs_filtered(&self.pool, since, level, limit).await
    }

    pub async fn count_logs_by_level(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<LevelCount>> {
        log_summary_queries::count_logs_by_level(&self.pool, since).await
    }

    pub async fn top_modules(
        &self,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<ModuleCount>> {
        log_summary_queries::top_modules(&self.pool, since, limit).await
    }

    pub async fn log_time_range(&self, since: Option<DateTime<Utc>>) -> Result<LogTimeRange> {
        log_summary_queries::log_time_range(&self.pool, since).await
    }

    pub async fn total_log_count(&self) -> Result<i64> {
        log_summary_queries::total_log_count(&self.pool).await
    }
}
