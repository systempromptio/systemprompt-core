use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;

use super::ai_trace_queries;
use super::models::{
    AiRequestInfo, ConversationMessage, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo,
    ToolLogEntry,
};

#[derive(Debug, Clone)]
pub struct AiTraceService {
    pool: Arc<PgPool>,
}

impl AiTraceService {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn resolve_task_id(&self, partial_id: &str) -> Result<String> {
        ai_trace_queries::resolve_task_id(&self.pool, partial_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No task found matching: {}", partial_id))
    }

    pub async fn get_task_info(&self, task_id: &str) -> Result<TaskInfo> {
        ai_trace_queries::fetch_task_info(&self.pool, task_id)
            .await
            .context("Failed to fetch task info")
    }

    pub async fn get_user_input(&self, task_id: &str) -> Result<Option<String>> {
        ai_trace_queries::fetch_user_input(&self.pool, task_id).await
    }

    pub async fn get_agent_response(&self, task_id: &str) -> Result<Option<String>> {
        ai_trace_queries::fetch_agent_response(&self.pool, task_id).await
    }

    pub async fn get_execution_steps(&self, task_id: &str) -> Result<Vec<ExecutionStep>> {
        ai_trace_queries::fetch_execution_steps(&self.pool, task_id).await
    }

    pub async fn get_ai_requests(&self, task_id: &str) -> Result<Vec<AiRequestInfo>> {
        ai_trace_queries::fetch_ai_requests(&self.pool, task_id).await
    }

    pub async fn get_system_prompt(&self, request_id: &str) -> Result<Option<String>> {
        ai_trace_queries::fetch_system_prompt(&self.pool, request_id).await
    }

    pub async fn get_conversation_messages(
        &self,
        request_id: &str,
    ) -> Result<Vec<ConversationMessage>> {
        ai_trace_queries::fetch_conversation_messages(&self.pool, request_id).await
    }

    pub async fn get_mcp_executions(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<McpToolExecution>> {
        ai_trace_queries::fetch_mcp_executions(&self.pool, task_id, context_id).await
    }

    pub async fn get_mcp_linked_ai_requests(
        &self,
        mcp_execution_id: &str,
    ) -> Result<Vec<AiRequestInfo>> {
        ai_trace_queries::fetch_mcp_linked_ai_requests(&self.pool, mcp_execution_id).await
    }

    pub async fn get_ai_request_message_previews(
        &self,
        request_id: &str,
    ) -> Result<Vec<ConversationMessage>> {
        ai_trace_queries::fetch_ai_request_message_previews(&self.pool, request_id).await
    }

    pub async fn get_tool_logs(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<ToolLogEntry>> {
        ai_trace_queries::fetch_tool_logs(&self.pool, task_id, context_id).await
    }

    pub async fn get_task_artifacts(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<TaskArtifact>> {
        ai_trace_queries::fetch_task_artifacts(&self.pool, task_id, context_id).await
    }
}
