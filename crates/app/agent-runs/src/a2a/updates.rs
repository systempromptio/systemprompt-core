//! [`TaskUpdate`]: the A2A streaming updates one [`RunEvent`] produces.
//!
//! Core has the two A2A streaming payloads, `TaskStatusUpdateEvent` and
//! `TaskArtifactUpdateEvent`, as separate structs, so [`TaskUpdate`] is the
//! one enum over both. `init` turns the task `Working`, a `runner` line is a
//! `Working` status carrying the line as an agent message, `assistant_text`
//! is a text artifact per event, and `result` emits its structured output as
//! a data artifact followed by the final `Completed`/`Failed` status. Other
//! kinds (tool calls, hooks, stderr) stay in the run's own event log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::models::a2a::protocol::{TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
use systemprompt_identifiers::{AgentRunId, ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{Message, MessageRole, Part, TaskState, TaskStatus};

use super::content::{data_part, outcome_artifact_id, run_artifact};
use crate::event::{RunEvent, RunEventPayload};

#[derive(Debug, Clone, PartialEq)]
pub enum TaskUpdate {
    Status(Box<TaskStatusUpdateEvent>),
    Artifact(Box<TaskArtifactUpdateEvent>),
}

struct Target<'a> {
    run_id: &'a AgentRunId,
    task_id: &'a TaskId,
    context_id: &'a ContextId,
    event: &'a RunEvent,
}

impl Target<'_> {
    fn status(&self, state: TaskState, message: Option<Message>) -> TaskUpdate {
        let status = TaskStatus {
            state,
            message,
            timestamp: Some(self.event.created_at),
        };
        TaskUpdate::Status(Box::new(TaskStatusUpdateEvent::new(
            self.task_id.clone(),
            self.context_id.clone(),
            status,
        )))
    }

    fn artifact(&self, id: ArtifactId, part: Part) -> TaskUpdate {
        let artifact = run_artifact(id, vec![part], self.task_id, self.context_id);
        TaskUpdate::Artifact(Box::new(TaskArtifactUpdateEvent::new(
            self.task_id.clone(),
            self.context_id.clone(),
            artifact,
        )))
    }

    fn agent_message(&self, text: &str) -> Message {
        Message {
            role: MessageRole::Agent,
            parts: vec![Part::text(text)],
            message_id: MessageId::generate(),
            task_id: Some(self.task_id.clone()),
            context_id: self.context_id.clone(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        }
    }
}

#[must_use]
pub fn task_events(
    run_id: &AgentRunId,
    task_id: &TaskId,
    context_id: &ContextId,
    event: &RunEvent,
) -> Vec<TaskUpdate> {
    let target = Target {
        run_id,
        task_id,
        context_id,
        event,
    };
    match &event.payload {
        RunEventPayload::Init { .. } => vec![target.status(TaskState::Working, None)],
        RunEventPayload::Runner { message, .. } => {
            let message = target.agent_message(message);
            vec![target.status(TaskState::Working, Some(message))]
        },
        RunEventPayload::AssistantText { text } => {
            let id = ArtifactId::new(format!("{}-text-{}", target.run_id, event.seq));
            let part = Part::text(text.clone());
            vec![target.artifact(id, part)]
        },
        RunEventPayload::Result {
            is_error,
            structured_output,
            error,
            ..
        } => {
            let mut updates = Vec::with_capacity(2);
            if let Some(value) = structured_output {
                let id = outcome_artifact_id(target.run_id);
                updates.push(target.artifact(id, data_part(value)));
            }
            let (state, message) = if *is_error {
                let text = error.as_deref().unwrap_or("run failed");
                (TaskState::Failed, Some(target.agent_message(text)))
            } else {
                (TaskState::Completed, None)
            };
            updates.push(target.status(state, message));
            updates
        },
        RunEventPayload::ToolUse { .. }
        | RunEventPayload::ToolResult { .. }
        | RunEventPayload::Hook { .. }
        | RunEventPayload::Stderr { .. } => Vec::new(),
    }
}
