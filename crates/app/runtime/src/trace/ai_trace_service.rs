//! Aggregated AI trace assembly across request, tool, and log rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AiRequestId, ContextId, McpExecutionId, TaskId};

use super::TraceError;
use super::models::{
    AiRequestInfo, ConversationMessage, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo,
    ToolLogEntry,
};
use super::repository::TraceRepository;

pub(super) type Result<T> = std::result::Result<T, TraceError>;

#[derive(Debug, Clone)]
pub struct AiTraceService {
    repository: TraceRepository,
}

impl AiTraceService {
    pub const fn new(repository: TraceRepository) -> Self {
        Self { repository }
    }

    pub async fn resolve_task_id(&self, partial_id: &str) -> Result<TaskId> {
        self.repository
            .resolve_task_id(partial_id)
            .await?
            .map(TaskId::new)
            .ok_or_else(|| TraceError::TaskNotFound {
                partial_id: partial_id.to_owned(),
            })
    }

    pub async fn get_task_info(&self, task_id: &TaskId) -> Result<TaskInfo> {
        self.repository.fetch_task_info(task_id).await
    }

    pub async fn get_user_input(&self, task_id: &TaskId) -> Result<Option<String>> {
        self.repository.fetch_user_input(task_id).await
    }

    pub async fn get_agent_response(&self, task_id: &TaskId) -> Result<Option<String>> {
        self.repository.fetch_agent_response(task_id).await
    }

    pub async fn get_execution_steps(&self, task_id: &TaskId) -> Result<Vec<ExecutionStep>> {
        self.repository.fetch_execution_steps(task_id).await
    }

    pub async fn get_ai_requests(&self, task_id: &TaskId) -> Result<Vec<AiRequestInfo>> {
        self.repository.fetch_ai_requests(task_id).await
    }

    pub async fn get_system_prompt(&self, request_id: &AiRequestId) -> Result<Option<String>> {
        self.repository.fetch_system_prompt(request_id).await
    }

    pub async fn get_conversation_messages(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<ConversationMessage>> {
        self.repository
            .fetch_conversation_messages(request_id)
            .await
    }

    pub async fn get_mcp_executions(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
    ) -> Result<Vec<McpToolExecution>> {
        self.repository
            .fetch_mcp_executions(task_id, context_id)
            .await
    }

    pub async fn get_mcp_linked_ai_requests(
        &self,
        mcp_execution_id: &McpExecutionId,
    ) -> Result<Vec<AiRequestInfo>> {
        self.repository
            .fetch_mcp_linked_ai_requests(mcp_execution_id)
            .await
    }

    pub async fn get_ai_request_message_previews(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<ConversationMessage>> {
        self.repository
            .fetch_ai_request_message_previews(request_id)
            .await
    }

    pub async fn get_tool_logs(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
    ) -> Result<Vec<ToolLogEntry>> {
        self.repository.fetch_tool_logs(task_id, context_id).await
    }

    pub async fn get_task_artifacts(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
    ) -> Result<Vec<TaskArtifact>> {
        self.repository
            .fetch_task_artifacts(task_id, context_id)
            .await
    }
}
