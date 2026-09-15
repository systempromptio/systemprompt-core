//! The request and task shapes the non-streaming handler works with: the
//! handler's parameters and the task/message values it builds around an
//! inbound message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use uuid::Uuid;

use crate::models::a2a::{Message, MessageRole, Part, Task, TaskState, TaskStatus, TextPart};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::{RequestContext, TaskMetadata};

use crate::services::a2a_server::active_tasks::ActiveTasks;

pub struct HandleMessageParams<'a> {
    pub message: Message,
    pub agent_runtime: &'a crate::models::AgentRuntimeInfo,
    pub agent_name: &'a str,
    pub context: &'a RequestContext,
    pub active_tasks: &'a ActiveTasks,
}

impl std::fmt::Debug for HandleMessageParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandleMessageParams")
            .field("agent_name", &self.agent_name)
            .field("task_id", &self.message.task_id)
            .finish_non_exhaustive()
    }
}


pub fn resolve_task_id(message: &Message) -> TaskId {
    message.task_id.clone().map_or_else(
        || {
            let new_task_id = TaskId::new(Uuid::new_v4().to_string());
            tracing::info!(task_id = %new_task_id, "Starting NEW task with generated ID");
            new_task_id
        },
        |existing_task_id| {
            tracing::info!(task_id = %existing_task_id, "Continuing existing task");
            existing_task_id
        },
    )
}

pub fn new_submitted_task(task_id: &TaskId, context_id: &ContextId, agent_name: &str) -> Task {
    Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: Some(TaskMetadata::new_agent_message(agent_name.to_owned())),
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

pub fn resolve_agent_message(task: &Task, user_message: &Message, response_text: &str) -> Message {
    task.status.message.clone().unwrap_or_else(|| {
        let client_message_id = user_message
            .metadata
            .as_ref()
            .and_then(|m| m.get("clientMessageId"))
            .cloned();

        let metadata = client_message_id.map(|id| serde_json::json!({"clientMessageId": id}));

        Message {
            role: MessageRole::Agent,
            parts: vec![Part::Text(TextPart {
                text: response_text.to_owned(),
            })],
            message_id: MessageId::generate(),
            task_id: Some(task.id.clone()),
            context_id: task.context_id.clone(),
            metadata,
            extensions: None,
            reference_task_ids: None,
        }
    })
}
