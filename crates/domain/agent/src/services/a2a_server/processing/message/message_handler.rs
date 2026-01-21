use anyhow::{anyhow, Result};
use uuid::Uuid;

use super::persistence::{broadcast_completion, persist_completed_task};
use super::stream_processor::StreamProcessor;
use super::{MessageProcessor, StreamEvent};
use crate::models::a2a::{Message, Part, Task, TaskState, TaskStatus, TextPart};
use crate::services::a2a_server::processing::task_builder::build_completed_task;
use crate::services::a2a_server::streaming::broadcast::broadcast_task_created;
use crate::services::a2a_server::streaming::webhook_client::broadcast_agui_event;
use systemprompt_identifiers::{MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{AgUiEventBuilder, AgUiMessageRole, RequestContext, TaskMetadata};

impl MessageProcessor {
    pub async fn handle_message(
        &self,
        message: Message,
        agent_name: &str,
        context: &RequestContext,
    ) -> Result<Task> {
        tracing::info!(agent_name = %agent_name, "Handling non-streaming message");

        let agent_runtime = self.load_agent_runtime(agent_name).await?;

        self.context_repo
            .get_context(&message.context_id, context.user_id())
            .await
            .map_err(|e| {
                anyhow!(
                    "Context validation failed - context_id: {}, user_id: {}, error: {}",
                    message.context_id,
                    context.user_id(),
                    e
                )
            })?;

        tracing::info!(
            context_id = %message.context_id,
            user_id = %context.user_id(),
            "Context validated"
        );

        let task_id = match message.task_id.clone() {
            Some(existing_task_id) => {
                tracing::info!(task_id = %existing_task_id, "Continuing existing task");
                existing_task_id
            },
            None => {
                let new_task_id = TaskId::new(Uuid::new_v4().to_string());
                tracing::info!(task_id = %new_task_id, "Starting NEW task with generated ID");
                new_task_id
            },
        };

        let metadata = TaskMetadata::new_agent_message(agent_name.to_string());

        let task = Task {
            id: task_id.clone(),
            context_id: message.context_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: Some(metadata),
            kind: "task".to_string(),
        };

        if let Err(e) = self
            .task_repo
            .create_task(
                &task,
                &UserId::new(context.user_id().as_str()),
                &SessionId::new(context.session_id().as_str()),
                &TraceId::new(context.trace_id().as_str()),
                agent_name,
            )
            .await
        {
            return Err(anyhow!("Failed to persist task at start: {}", e));
        }

        tracing::info!(task_id = %task_id, "Task persisted to database");

        broadcast_task_created(
            &task_id,
            &message.context_id,
            context.user_id().as_str(),
            &message,
            agent_name,
            context.auth_token().as_str(),
        )
        .await;

        let working_timestamp = chrono::Utc::now();
        if let Err(e) = self
            .task_repo
            .update_task_state(&task_id, TaskState::Working, &working_timestamp)
            .await
        {
            tracing::error!(task_id = %task_id, error = %e, "Failed to mark task as working");
        }

        let stream_processor = StreamProcessor {
            ai_service: self.ai_service.clone(),
            context_service: self.context_service.clone(),
            skill_service: self.skill_service.clone(),
            execution_step_repo: self.execution_step_repo.clone(),
        };

        let mut chunk_rx = stream_processor
            .process_message_stream(
                &message,
                &agent_runtime,
                agent_name,
                context,
                task_id.clone(),
            )
            .await?;

        let mut response_text = String::new();
        let mut tool_artifacts = Vec::new();

        while let Some(event) = chunk_rx.recv().await {
            match event {
                StreamEvent::Text(text) => {
                    response_text.push_str(&text);
                },
                StreamEvent::Complete {
                    full_text,
                    artifacts,
                } => {
                    response_text = full_text;
                    tool_artifacts = artifacts;
                },
                StreamEvent::Error(error) => {
                    let error_event = AgUiEventBuilder::run_error(
                        error.clone(),
                        Some("EXECUTION_ERROR".to_string()),
                    );
                    if let Err(e) = broadcast_agui_event(
                        context.user_id().as_str(),
                        error_event,
                        context.auth_token().as_str(),
                    )
                    .await
                    {
                        tracing::debug!(error = %e, "Failed to broadcast error event");
                    }
                    return Err(anyhow!(error));
                },
                _ => {},
            }
        }

        let task = build_completed_task(
            task_id,
            message.context_id.clone(),
            response_text.clone(),
            message.clone(),
            tool_artifacts,
        );

        let agent_message = task.status.message.clone().unwrap_or_else(|| {
            let client_message_id = message
                .metadata
                .as_ref()
                .and_then(|m| m.get("clientMessageId"))
                .cloned();

            let metadata = client_message_id.map(|id| serde_json::json!({"clientMessageId": id}));

            Message {
                role: "agent".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: response_text.clone(),
                })],
                id: MessageId::generate(),
                task_id: Some(task.id.clone()),
                context_id: task.context_id.clone(),
                kind: "message".to_string(),
                metadata,
                extensions: None,
                reference_task_ids: None,
            }
        });

        if context.user_type() == systemprompt_models::auth::UserType::Anon {
            tracing::warn!(
                context_id = %message.context_id,
                session_id = %context.session_id(),
                "Saving messages for anonymous user"
            );
        }

        if let Err(e) = persist_completed_task(
            &task,
            &message,
            &agent_message,
            context,
            &self.task_repo,
            &self.db_pool,
            false,
        )
        .await
        {
            let error_msg = format!("Failed to persist completed task: {}", e);
            tracing::error!(task_id = %task.id, error = %e, "Failed to persist completed task");

            let failed_timestamp = chrono::Utc::now();
            if let Err(update_err) = self
                .task_repo
                .update_task_failed_with_error(&task.id, &error_msg, &failed_timestamp)
                .await
            {
                tracing::error!(task_id = %task.id, error = %update_err, "Failed to update task to failed state");
            }

            return Err(e);
        }

        broadcast_completion(&task, context).await;

        let user_id = context.user_id().as_str();
        let auth_token = context.auth_token().as_str();
        let context_id = task.context_id.clone();
        let task_id = task.id.clone();
        let message_id = agent_message.id.clone();

        let start_event = AgUiEventBuilder::run_started(context_id.clone(), task_id.clone(), None);
        if let Err(e) = broadcast_agui_event(user_id, start_event, auth_token).await {
            tracing::debug!(error = %e, "Failed to broadcast run_started event");
        }

        let msg_start = AgUiEventBuilder::text_message_start(
            message_id.to_string(),
            AgUiMessageRole::Assistant,
        );
        if let Err(e) = broadcast_agui_event(user_id, msg_start, auth_token).await {
            tracing::debug!(error = %e, "Failed to broadcast text_message_start event");
        }

        let msg_content =
            AgUiEventBuilder::text_message_content(message_id.to_string(), &response_text);
        if let Err(e) = broadcast_agui_event(user_id, msg_content, auth_token).await {
            tracing::debug!(error = %e, "Failed to broadcast text_message_content event");
        }

        let msg_end = AgUiEventBuilder::text_message_end(message_id.to_string());
        if let Err(e) = broadcast_agui_event(user_id, msg_end, auth_token).await {
            tracing::debug!(error = %e, "Failed to broadcast text_message_end event");
        }

        let result = serde_json::json!({
            "text": response_text,
            "artifacts": task.artifacts,
        });
        let finish_event = AgUiEventBuilder::run_finished(context_id, task_id, Some(result));
        if let Err(e) = broadcast_agui_event(user_id, finish_event, auth_token).await {
            tracing::debug!(error = %e, "Failed to broadcast run_finished event");
        }

        Ok(task)
    }
}
