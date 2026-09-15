//! Persisting A2A conversation messages, including synthetic messages for
//! MCP tool executions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::Result;
use serde_json::json;

use crate::models::a2a::{Message, MessageRole, Part, TextPart};
use crate::repository::task::{PersistMessagesTxParams, TaskRepository};
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::RequestContext;

#[derive(Debug)]
pub struct PersistMessagesParams<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub messages: Vec<Message>,
    pub user_id: Option<&'a UserId>,
    pub session_id: &'a SessionId,
    pub trace_id: &'a TraceId,
}

#[derive(Debug)]
pub struct CreateToolExecutionMessageParams<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub tool_name: &'a str,
    pub tool_args: &'a serde_json::Value,
    pub request_context: &'a RequestContext,
}

pub struct MessageService {
    task_repo: TaskRepository,
}

impl std::fmt::Debug for MessageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageService").finish_non_exhaustive()
    }
}

impl MessageService {
    #[must_use]
    pub const fn new(task_repo: TaskRepository) -> Self {
        Self { task_repo }
    }

    pub async fn persist_messages(&self, params: PersistMessagesParams<'_>) -> Result<Vec<i32>> {
        let PersistMessagesParams {
            task_id,
            context_id,
            messages,
            user_id,
            session_id,
            trace_id,
        } = params;

        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let sequence_numbers = self
            .task_repo
            .persist_messages(PersistMessagesTxParams {
                task_id,
                context_id,
                messages: &messages,
                user_id,
                session_id,
                trace_id,
            })
            .await?;

        tracing::info!(
            task_id = %task_id,
            sequence_numbers = ?sequence_numbers,
            "Messages persisted"
        );

        Ok(sequence_numbers)
    }

    pub async fn create_tool_execution_message(
        &self,
        params: CreateToolExecutionMessageParams<'_>,
    ) -> Result<(MessageId, i32)> {
        let CreateToolExecutionMessageParams {
            task_id,
            context_id,
            tool_name,
            tool_args,
            request_context,
        } = params;
        let message_id = MessageId::generate();

        let tool_args_display =
            serde_json::to_string_pretty(tool_args).unwrap_or_else(|_| tool_args.to_string());

        let timestamp = chrono::Utc::now().to_rfc3339();

        let message = Message {
            role: MessageRole::User,
            message_id: message_id.clone(),
            task_id: Some(task_id.clone()),
            context_id: context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: format!(
                    "Executed MCP tool: {} with arguments:\n{}\n\nExecution ID: {} at {}",
                    tool_name,
                    tool_args_display,
                    task_id.as_str(),
                    timestamp
                ),
            })],
            metadata: Some(json!({
                "source": "mcp_direct_call",
                "tool_name": tool_name,
                "is_synthetic": true,
                "tool_args": tool_args,
                "execution_timestamp": timestamp,
            })),
            extensions: None,
            reference_task_ids: None,
        };

        let sequence_numbers = self
            .task_repo
            .persist_messages(PersistMessagesTxParams {
                task_id,
                context_id,
                messages: std::slice::from_ref(&message),
                user_id: Some(request_context.user_id()),
                session_id: request_context.session_id(),
                trace_id: request_context.trace_id(),
            })
            .await?;
        let sequence_number = sequence_numbers.first().copied().unwrap_or(0);

        tracing::info!(
            message_id = %message_id,
            task_id = %task_id,
            tool_name = %tool_name,
            sequence_number = sequence_number,
            "Created synthetic tool execution message"
        );

        Ok((message_id, sequence_number))
    }
}
