use anyhow::{anyhow, Result};
use serde_json::json;
use uuid::Uuid;

use crate::models::a2a::{Message, Part, TextPart};
use crate::repository::task::TaskRepository;
use systemprompt_core_database::{DatabaseProvider, DatabaseTransaction, DbPool};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::RequestContext;
use systemprompt_traits::Repository;

pub struct MessageService {
    task_repo: TaskRepository,
}

impl std::fmt::Debug for MessageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageService").finish_non_exhaustive()
    }
}

impl MessageService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            task_repo: TaskRepository::new(db_pool),
        }
    }

    pub async fn persist_message_in_tx(
        &self,
        tx: &mut dyn DatabaseTransaction,
        message: &Message,
        task_id: &TaskId,
        context_id: &ContextId,
        user_id: Option<&systemprompt_identifiers::UserId>,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
    ) -> Result<i32> {
        let sequence_number = self
            .task_repo
            .get_next_sequence_number_in_tx(tx, task_id)
            .await?;

        self.task_repo
            .persist_message_with_tx(
                tx,
                message,
                task_id,
                context_id,
                sequence_number,
                user_id,
                session_id,
                trace_id,
            )
            .await
            .map_err(|e| anyhow!("Failed to persist message: {}", e))?;

        tracing::info!(
            message_id = %message.id,
            task_id = %task_id,
            sequence_number = sequence_number,
            "Message persisted"
        );

        Ok(sequence_number)
    }

    pub async fn persist_messages(
        &self,
        task_id: &TaskId,
        context_id: &ContextId,
        messages: Vec<Message>,
        user_id: Option<&systemprompt_identifiers::UserId>,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
    ) -> Result<Vec<i32>> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.task_repo.pool().as_ref().begin_transaction().await?;
        let mut sequence_numbers = Vec::new();

        tracing::info!(
            task_id = %task_id,
            message_count = messages.len(),
            "Persisting multiple messages"
        );

        for message in messages {
            let seq = self
                .persist_message_in_tx(
                    &mut *tx, &message, task_id, context_id, user_id, session_id, trace_id,
                )
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
        task_id: &TaskId,
        context_id: &ContextId,
        tool_name: &str,
        tool_args: &serde_json::Value,
        request_context: &RequestContext,
    ) -> Result<(String, i32)> {
        let message_id = Uuid::new_v4().to_string();

        let tool_args_display =
            serde_json::to_string_pretty(tool_args).unwrap_or_else(|_| tool_args.to_string());

        let timestamp = chrono::Utc::now().to_rfc3339();

        let message = Message {
            role: "user".to_string(),
            id: message_id.clone().into(),
            task_id: Some(task_id.clone()),
            context_id: context_id.clone(),
            kind: "message".to_string(),
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

        let mut tx = self.task_repo.pool().as_ref().begin_transaction().await?;

        let sequence_number = self
            .persist_message_in_tx(
                &mut *tx,
                &message,
                task_id,
                context_id,
                Some(request_context.user_id()),
                request_context.session_id(),
                request_context.trace_id(),
            )
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
