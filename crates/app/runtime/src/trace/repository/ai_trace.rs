//! Queries assembling an AI task trace from the database.
//!
//! Fetches the components of a single agent task — task info, user input and
//! agent response, execution steps, AI requests with their system prompt and
//! conversation messages — and re-exports the MCP-execution query counterparts.
//! `resolve_task_id` expands a partial task id to the most recent full match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentName, AiRequestId, ContextId, ExecutionStepId, TaskId};

use super::{Result, TraceRepository};
use crate::trace::models::{AiRequestInfo, ConversationMessage, ExecutionStep, TaskInfo};

impl TraceRepository {
    pub async fn resolve_task_id(&self, partial_id: &str) -> Result<Option<String>> {
        let pattern = format!("{}%", partial_id);
        let row = sqlx::query!(
        "SELECT task_id FROM agent_tasks WHERE task_id LIKE $1 ORDER BY created_at DESC LIMIT 1",
        pattern
    )
    .fetch_optional(&*self.pool)
    .await?;

        Ok(row.map(|r| r.task_id))
    }

    pub async fn fetch_task_info(&self, task_id: &TaskId) -> Result<TaskInfo> {
        let row = sqlx::query!(
            r#"SELECT task_id, context_id, agent_name, status, created_at, started_at, completed_at,
                  execution_time_ms, error_message
           FROM agent_tasks WHERE task_id = $1"#,
            task_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(TaskInfo {
            task_id: TaskId::new(row.task_id),
            context_id: ContextId::try_new(&row.context_id)?,
            agent_name: row.agent_name.map(AgentName::new),
            status: row.status,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            execution_time_ms: row.execution_time_ms,
            error_message: row.error_message,
        })
    }

    pub async fn fetch_user_input(&self, task_id: &TaskId) -> Result<Option<String>> {
        let row = sqlx::query!(
            r#"SELECT mp.text_content
           FROM task_messages tm
           JOIN message_parts mp ON mp.message_id = tm.message_id AND mp.task_id = tm.task_id
           WHERE tm.task_id = $1 AND tm.role = 'user' AND mp.part_kind = 'text'
           ORDER BY tm.sequence_number DESC LIMIT 1"#,
            task_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.and_then(|r| r.text_content))
    }

    pub async fn fetch_agent_response(&self, task_id: &TaskId) -> Result<Option<String>> {
        let row = sqlx::query!(
            r#"SELECT mp.text_content
           FROM task_messages tm
           JOIN message_parts mp ON mp.message_id = tm.message_id AND mp.task_id = tm.task_id
           WHERE tm.task_id = $1 AND tm.role = 'agent' AND mp.part_kind = 'text'
           ORDER BY tm.sequence_number DESC LIMIT 1"#,
            task_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.and_then(|r| r.text_content))
    }

    pub async fn fetch_execution_steps(&self, task_id: &TaskId) -> Result<Vec<ExecutionStep>> {
        let rows = sqlx::query!(
            r#"SELECT
               step_id as id,
               content->>'type' as step_type,
               COALESCE(content->>'title', content->>'type') as title,
               status,
               duration_ms,
               error_message
           FROM task_execution_steps
           WHERE task_id = $1
           ORDER BY started_at"#,
            task_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ExecutionStep {
                step_id: ExecutionStepId::new(r.id),
                step_type: r.step_type,
                title: r.title,
                status: r.status,
                duration_ms: r.duration_ms,
                error_message: r.error_message,
            })
            .collect())
    }

    pub async fn fetch_ai_requests(&self, task_id: &TaskId) -> Result<Vec<AiRequestInfo>> {
        let rows = sqlx::query!(
        r#"SELECT id, model, provider, max_tokens, input_tokens, output_tokens, cost_microdollars, latency_ms
           FROM ai_requests
           WHERE task_id = $1
           ORDER BY created_at"#,
        task_id.as_str()
    )
    .fetch_all(&*self.pool)
    .await?;

        Ok(rows
            .into_iter()
            .map(|r| AiRequestInfo {
                id: AiRequestId::new(r.id),
                provider: r.provider,
                model: r.model,
                max_tokens: r.max_tokens,
                input_tokens: r.input_tokens,
                output_tokens: r.output_tokens,
                cost_microdollars: r.cost_microdollars,
                latency_ms: r.latency_ms,
            })
            .collect())
    }

    pub async fn fetch_system_prompt(&self, request_id: &AiRequestId) -> Result<Option<String>> {
        let row = sqlx::query!(
            r#"SELECT content
           FROM ai_request_messages
           WHERE request_id = $1 AND role = 'system' AND sequence_number = 0
           LIMIT 1"#,
            request_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| r.content))
    }

    pub async fn fetch_conversation_messages(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<ConversationMessage>> {
        let rows = sqlx::query!(
            r#"SELECT role, content, sequence_number
           FROM ai_request_messages
           WHERE request_id = $1
           ORDER BY sequence_number"#,
            request_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ConversationMessage {
                role: r.role,
                content: r.content,
                sequence_number: r.sequence_number,
            })
            .collect())
    }

    pub async fn fetch_ai_request_message_previews(
        &self,
        request_id: &AiRequestId,
    ) -> Result<Vec<ConversationMessage>> {
        let rows = sqlx::query!(
            r#"SELECT role, LEFT(content, 500) as content_preview, sequence_number
           FROM ai_request_messages
           WHERE request_id = $1
           ORDER BY sequence_number"#,
            request_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ConversationMessage {
                role: r.role,
                content: r.content_preview.unwrap_or_else(String::new),
                sequence_number: r.sequence_number,
            })
            .collect())
    }
}
