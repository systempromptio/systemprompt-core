use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::TaskId;

use super::ai_trace_queries;
use super::models::{
    AiRequestInfo, ConversationMessage, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo,
    ToolLogEntry,
};
use crate::models::LoggingError;

type Result<T> = std::result::Result<T, LoggingError>;

/// Service exposing trace queries for AI request lifecycles, conversations, MCP
/// tool calls, and task artifacts. Backed by a Postgres connection pool.
#[derive(Debug, Clone)]
pub struct AiTraceService {
    pool: Arc<PgPool>,
}

impl AiTraceService {
    /// Construct a new AI trace service from an existing Postgres pool.
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Resolve a (possibly partial) task identifier prefix to a full
    /// [`TaskId`].
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::TaskNotFound`] when no task matches the prefix,
    /// and [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn resolve_task_id(&self, partial_id: &str) -> Result<TaskId> {
        ai_trace_queries::resolve_task_id(&self.pool, partial_id)
            .await?
            .map(TaskId::from)
            .ok_or_else(|| LoggingError::TaskNotFound {
                partial_id: partial_id.to_string(),
            })
    }

    /// Fetch the canonical task info row for a given task id.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_task_info(&self, task_id: &TaskId) -> Result<TaskInfo> {
        ai_trace_queries::fetch_task_info(&self.pool, task_id).await
    }

    /// Fetch the user input prompt that initiated the task, if any.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_user_input(&self, task_id: &TaskId) -> Result<Option<String>> {
        ai_trace_queries::fetch_user_input(&self.pool, task_id).await
    }

    /// Fetch the agent's final response text for a task, if any.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_agent_response(&self, task_id: &TaskId) -> Result<Option<String>> {
        ai_trace_queries::fetch_agent_response(&self.pool, task_id).await
    }

    /// Fetch the ordered list of execution steps recorded for a task.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_execution_steps(&self, task_id: &TaskId) -> Result<Vec<ExecutionStep>> {
        ai_trace_queries::fetch_execution_steps(&self.pool, task_id).await
    }

    /// Fetch summaries of all AI requests issued during a task.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_ai_requests(&self, task_id: &TaskId) -> Result<Vec<AiRequestInfo>> {
        ai_trace_queries::fetch_ai_requests(&self.pool, task_id).await
    }

    /// Fetch the system prompt sent with a specific AI request, if any.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_system_prompt(&self, request_id: &str) -> Result<Option<String>> {
        ai_trace_queries::fetch_system_prompt(&self.pool, request_id).await
    }

    /// Fetch the full conversation message history attached to an AI request.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_conversation_messages(
        &self,
        request_id: &str,
    ) -> Result<Vec<ConversationMessage>> {
        ai_trace_queries::fetch_conversation_messages(&self.pool, request_id).await
    }

    /// List MCP tool executions associated with a specific task and context.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_mcp_executions(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<McpToolExecution>> {
        ai_trace_queries::fetch_mcp_executions(&self.pool, task_id, context_id).await
    }

    /// List AI requests linked to a given MCP tool execution.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_mcp_linked_ai_requests(
        &self,
        mcp_execution_id: &str,
    ) -> Result<Vec<AiRequestInfo>> {
        ai_trace_queries::fetch_mcp_linked_ai_requests(&self.pool, mcp_execution_id).await
    }

    /// Fetch short message previews for the conversation attached to an AI
    /// request.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_ai_request_message_previews(
        &self,
        request_id: &str,
    ) -> Result<Vec<ConversationMessage>> {
        ai_trace_queries::fetch_ai_request_message_previews(&self.pool, request_id).await
    }

    /// Fetch tool log entries for a given task and context.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_tool_logs(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<ToolLogEntry>> {
        ai_trace_queries::fetch_tool_logs(&self.pool, task_id, context_id).await
    }

    /// Fetch artifacts produced by a task within the given context.
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::DatabaseError`] when the underlying query fails.
    pub async fn get_task_artifacts(
        &self,
        task_id: &str,
        context_id: &str,
    ) -> Result<Vec<TaskArtifact>> {
        ai_trace_queries::fetch_task_artifacts(&self.pool, task_id, context_id).await
    }
}
