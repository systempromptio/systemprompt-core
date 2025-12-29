use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{AiRequestSummary, ExecutionStepSummary, McpExecutionSummary, TraceEvent};
use super::queries;

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
}
