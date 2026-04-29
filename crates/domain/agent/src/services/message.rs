use anyhow::{Result, anyhow};
use serde_json::json;
use uuid::Uuid;

use crate::models::a2a::{Message, MessageRole, Part, TextPart};
use crate::repository::context::message::PersistMessageWithTxParams;
use crate::repository::task::TaskRepository;
use systemprompt_database::{DatabaseProvider, DatabaseTransaction, DbPool};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::RequestContext;

pub struct PersistMessageInTxParams<'a> {
    pub tx: &'a mut dyn DatabaseTransaction,
    pub message: &'a Message,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub user_id: Option<&'a systemprompt_identifiers::UserId>,
    pub session_id: &'a systemprompt_identifiers::SessionId,
    pub trace_id: &'a systemprompt_identifiers::TraceId,
}

impl std::fmt::Debug for PersistMessageInTxParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistMessageInTxParams")
            .field("message", &self.message)
            .field("task_id", &self.task_id)
            .field("context_id", &self.context_id)
            .field("user_id", &self.user_id)
            .field("session_id", &self.session_id)
            .field("trace_id", &self.trace_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct PersistMessagesParams<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub messages: Vec<Message>,
    pub user_id: Option<&'a systemprompt_identifiers::UserId>,
    pub session_id: &'a systemprompt_identifiers::SessionId,
    pub trace_id: &'a systemprompt_identifiers::TraceId,
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
    pub fn new(db_pool: &DbPool) -> Result<Self> {
        Ok(Self {
            task_repo: TaskRepository::new(db_pool)?,
        })
    }

    pub async fn persist_message_in_tx(&self, params: PersistMessageInTxParams<'_>) -> Result<i32> {
        let PersistMessageInTxParams {
            tx,
            message,
            task_id,
            context_id,
            user_id,
            session_id,
            trace_id,
        } = params;
        let sequence_number = self
            .task_repo
            .get_next_sequence_number_in_tx(tx, task_id)
            .await?;

        self.task_repo
            .persist_message_with_tx(PersistMessageWithTxParams {
                tx,
                message,
                task_id,
                context_id,
                sequence_number,
                user_id,
                session_id,
                trace_id,
            })
            .await
            .map_err(|e| anyhow!("Failed to persist message: {}", e))?;

        tracing::info!(
            message_id = %message.message_id,
            task_id = %task_id,
            sequence_number = sequence_number,
            "Message persisted"
        );

        Ok(sequence_number)
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

        let mut tx = self
            .task_repo
            .db_pool()
            .as_ref()
            .begin_transaction()
            .await?;
        let mut sequence_numbers = Vec::new();

        tracing::info!(
            task_id = %task_id,
            message_count = messages.len(),
            "Persisting multiple messages"
        );

        for message in messages {
            let seq = self
                .persist_message_in_tx(PersistMessageInTxParams {
                    tx: &mut *tx,
                    message: &message,
                    task_id,
                    context_id,
                    user_id,
                    session_id,
                    trace_id,
                })
                .await?;
            sequence_numbers.push(seq);
        }

        tx.commit().await?;

        tracing::info!(
            task_id = %task_id,
            sequence_numbers = ?sequence_numbers,
            "Messages persisted successfully"
        );

        Ok(sequence_numbers)
    }

    pub async fn create_tool_execution_message(
        &self,
        params: CreateToolExecutionMessageParams<'_>,
    ) -> Result<(String, i32)> {
        let CreateToolExecutionMessageParams {
            task_id,
            context_id,
            tool_name,
            tool_args,
            request_context,
        } = params;
        let message_id = Uuid::new_v4().to_string();

        let tool_args_display =
            serde_json::to_string_pretty(tool_args).unwrap_or_else(|_| tool_args.to_string());

        let timestamp = chrono::Utc::now().to_rfc3339();

        let message = Message {
            role: MessageRole::User,
            message_id: MessageId::new(message_id.clone()),
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

        let mut tx = self
            .task_repo
            .db_pool()
            .as_ref()
            .begin_transaction()
            .await?;

        let sequence_number = self
            .persist_message_in_tx(PersistMessageInTxParams {
                tx: &mut *tx,
                message: &message,
                task_id,
                context_id,
                user_id: Some(request_context.user_id()),
                session_id: request_context.session_id(),
                trace_id: request_context.trace_id(),
            })
            .await?;

        tx.commit().await?;

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
