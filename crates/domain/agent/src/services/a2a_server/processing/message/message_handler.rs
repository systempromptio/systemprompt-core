use std::sync::Arc;

use crate::services::shared::{AgentServiceError, Result};
use uuid::Uuid;

use super::persistence::{broadcast_completion, persist_completed_task};
use super::stream_processor::StreamProcessor;
use super::{MessageProcessor, StreamEvent};
use crate::models::a2a::{
    Artifact, Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart,
};
use crate::services::a2a_server::processing::task_builder::build_completed_task;
use crate::services::a2a_server::streaming::broadcast::{
    BroadcastTaskCreatedParams, broadcast_task_created,
};
use crate::services::a2a_server::streaming::webhook_client::broadcast_agui_event;
use systemprompt_identifiers::{MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::{AgUiEventBuilder, AgUiMessageRole, RequestContext, TaskMetadata};

impl MessageProcessor {
    pub(crate) async fn handle_message(
        &self,
        message: Message,
        agent_name: &str,
        context: &RequestContext,
    ) -> Result<Task> {
        tracing::info!(agent_name = %agent_name, "Handling non-streaming message");

        let agent_runtime = self.load_agent_runtime(agent_name).await?;

        let context_id = &message.context_id;

        self.context_repo
            .get_context(context_id, context.user_id())
            .await
            .map_err(|e| {
                AgentServiceError::Internal(format!(
                    "Context validation failed - context_id: {}, user_id: {}, error: {}",
                    context_id,
                    context.user_id(),
                    e
                ))
            })?;

        tracing::info!(
            context_id = %context_id,
            user_id = %context.user_id(),
            "Context validated"
        );

        let task_id = message.task_id.clone().map_or_else(
            || {
                let new_task_id = TaskId::new(Uuid::new_v4().to_string());
                tracing::info!(task_id = %new_task_id, "Starting NEW task with generated ID");
                new_task_id
            },
            |existing_task_id| {
                tracing::info!(task_id = %existing_task_id, "Continuing existing task");
                existing_task_id
            },
        );

        let metadata = TaskMetadata::new_agent_message(agent_name.to_owned());

        let task = Task {
            id: task_id.clone(),
            context_id: context_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: Some(metadata),
            created_at: Some(chrono::Utc::now()),
            last_modified: Some(chrono::Utc::now()),
        };

        if let Err(e) = self
            .task_repo
            .create_task(crate::repository::task::RepoCreateTaskParams {
                task: &task,
                user_id: &UserId::new(context.user_id().as_str()),
                session_id: &SessionId::new(context.session_id().as_str()),
                trace_id: &TraceId::new(context.trace_id().as_str()),
                agent_name,
            })
            .await
        {
            return Err(AgentServiceError::Internal(format!(
                "Failed to persist task at start: {e}"
            )));
        }

        tracing::info!(task_id = %task_id, "Task persisted to database");

        broadcast_task_created(BroadcastTaskCreatedParams {
            task_id: &task_id,
            context_id,
            user_id: context.user_id().as_str(),
            user_message: &message,
            agent_name,
            token: context.auth_token().as_str(),
        })
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
            ai_service: Arc::clone(&self.ai_service),
            context_service: self.context_service.clone(),
            skill_service: Arc::clone(&self.skill_service),
            execution_step_repo: Arc::clone(&self.execution_step_repo),
        };

        let chunk_rx = stream_processor
            .process_message_stream(super::ProcessMessageStreamParams {
                a2a_message: &message,
                agent_runtime: &agent_runtime,
                agent_name,
                context,
                task_id: task_id.clone(),
            })
            .await?;

        let (response_text, tool_artifacts) = collect_stream_response(chunk_rx, context).await?;

        let task = build_completed_task(
            task_id,
            context_id.clone(),
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
                role: MessageRole::Agent,
                parts: vec![Part::Text(TextPart {
                    text: response_text.clone(),
                })],
                message_id: MessageId::generate(),
                task_id: Some(task.id.clone()),
                context_id: task.context_id.clone(),
                metadata,
                extensions: None,
                reference_task_ids: None,
            }
        });

        if context.user_type() == systemprompt_models::auth::UserType::Anon {
            tracing::warn!(
                context_id = %context_id,
                session_id = %context.session_id(),
                "Saving messages for anonymous user"
            );
        }

        self.persist_or_mark_failed(&task, &message, &agent_message, context)
            .await?;

        broadcast_completion(&task, context).await;

        broadcast_agui_lifecycle(BroadcastAguiLifecycleParams {
            context,
            context_id,
            task: &task,
            agent_message: &agent_message,
            response_text: &response_text,
        })
        .await;

        Ok(task)
    }

    async fn persist_or_mark_failed(
        &self,
        task: &Task,
        user_message: &Message,
        agent_message: &Message,
        context: &RequestContext,
    ) -> Result<()> {
        let Err(e) = persist_completed_task(super::persistence::PersistCompletedTaskParams {
            task,
            user_message,
            agent_message,
            context,
            task_repo: &self.task_repo,
            db_pool: &self.db_pool,
            artifacts_already_published: false,
        })
        .await
        else {
            return Ok(());
        };

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

        Err(e)
    }
}

async fn collect_stream_response(
    mut chunk_rx: tokio::sync::mpsc::Receiver<StreamEvent>,
    context: &RequestContext,
) -> Result<(String, Vec<Artifact>)> {
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
                    Some("EXECUTION_ERROR".to_owned()),
                );
                if let Err(e) = broadcast_agui_event(
                    context.user_id(),
                    error_event,
                    context.auth_token().as_str(),
                )
                .await
                {
                    tracing::debug!(error = %e, "Failed to broadcast error event");
                }
                return Err(AgentServiceError::Internal(error.clone()));
            },
            _ => {},
        }
    }

    Ok((response_text, tool_artifacts))
}

struct BroadcastAguiLifecycleParams<'a> {
    context: &'a RequestContext,
    context_id: &'a systemprompt_identifiers::ContextId,
    task: &'a Task,
    agent_message: &'a Message,
    response_text: &'a str,
}

async fn broadcast_agui_lifecycle(params: BroadcastAguiLifecycleParams<'_>) {
    let user_id = params.context.user_id();
    let auth_token = params.context.auth_token().as_str();
    let task_id = params.task.id.clone();
    let message_id = params.agent_message.message_id.clone();

    let start_event =
        AgUiEventBuilder::run_started(params.context_id.clone(), task_id.clone(), None);
    if let Err(e) = broadcast_agui_event(user_id, start_event, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast run_started event");
    }

    let msg_start =
        AgUiEventBuilder::text_message_start(message_id.to_string(), AgUiMessageRole::Assistant);
    if let Err(e) = broadcast_agui_event(user_id, msg_start, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_start event");
    }

    let msg_content =
        AgUiEventBuilder::text_message_content(message_id.to_string(), params.response_text);
    if let Err(e) = broadcast_agui_event(user_id, msg_content, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_content event");
    }

    let msg_end = AgUiEventBuilder::text_message_end(message_id.to_string());
    if let Err(e) = broadcast_agui_event(user_id, msg_end, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast text_message_end event");
    }

    let result = serde_json::json!({
        "text": params.response_text,
        "artifacts": params.task.artifacts,
    });
    let finish_event =
        AgUiEventBuilder::run_finished(params.context_id.clone(), task_id, Some(result));
    if let Err(e) = broadcast_agui_event(user_id, finish_event, auth_token).await {
        tracing::debug!(error = %e, "Failed to broadcast run_finished event");
    }
}
