//! Persist user / agent messages produced by an MCP tool execution.

use crate::models::a2a::{Artifact, Message, MessageRole, Part, TextPart};
use crate::services::MessageService;
use rmcp::ErrorData as McpError;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};

/// Parameters for [`save_messages_for_tool_execution`].
#[derive(Debug)]
pub struct SaveMessagesForToolExecutionParams<'a> {
    /// Database pool.
    pub db_pool: &'a DbPool,
    /// Owning task id.
    pub task_id: &'a TaskId,
    /// Owning context id.
    pub context_id: &'a ContextId,
    /// MCP tool name that was executed.
    pub tool_name: &'a str,
    /// Stringified tool result.
    pub tool_result: &'a str,
    /// Optional artifact produced by the tool.
    pub artifact: Option<&'a Artifact>,
    /// User the tool was executed on behalf of.
    pub user_id: &'a UserId,
    /// Owning session id.
    pub session_id: &'a SessionId,
    /// Trace id propagated through the execution.
    pub trace_id: &'a TraceId,
}

/// Persist a synthetic user/agent message pair representing an MCP tool
/// execution.
///
/// # Errors
/// Returns [`McpError`] if the message service cannot be initialised or the
/// message persistence fails.
pub async fn save_messages_for_tool_execution(
    params: SaveMessagesForToolExecutionParams<'_>,
) -> Result<(), McpError> {
    let SaveMessagesForToolExecutionParams {
        db_pool,
        task_id,
        context_id,
        tool_name,
        tool_result,
        artifact,
        user_id,
        session_id,
        trace_id,
    } = params;
    let message_service = MessageService::new(db_pool).map_err(|e| {
        McpError::internal_error(format!("Failed to create message service: {e}"), None)
    })?;

    let user_message = Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: format!("Execute tool: {tool_name}"),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    let agent_text = artifact.map_or_else(
        || format!("Tool execution completed. Result: {tool_result}"),
        |artifact| {
            format!(
                "Tool execution completed. Result: {}\n\nArtifact created: {} (type: {})",
                tool_result, artifact.id, artifact.metadata.artifact_type
            )
        },
    );

    let agent_message = Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart { text: agent_text })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    message_service
        .persist_messages(crate::services::PersistMessagesParams {
            task_id,
            context_id,
            messages: vec![user_message, agent_message],
            user_id: Some(user_id),
            session_id,
            trace_id,
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to save messages: {e}"), None))?;

    Ok(())
}
