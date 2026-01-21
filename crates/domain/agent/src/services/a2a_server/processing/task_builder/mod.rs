mod builders;
mod helpers;

pub use builders::{
    build_canceled_task, build_completed_task, build_mock_task, build_multiturn_task,
    build_submitted_task,
};

use crate::models::a2a::{Artifact, Message, Part, Task, TaskState, TaskStatus, TextPart};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskMetadata;

#[derive(Debug)]
pub struct TaskBuilder {
    task_id: TaskId,
    context_id: ContextId,
    state: TaskState,
    response_text: String,
    id: MessageId,
    user_message: Option<Message>,
    artifacts: Vec<Artifact>,
    metadata: Option<TaskMetadata>,
}

impl TaskBuilder {
    pub fn new(context_id: ContextId) -> Self {
        Self {
            task_id: TaskId::generate(),
            context_id,
            state: TaskState::Completed,
            response_text: String::new(),
            id: MessageId::generate(),
            user_message: None,
            artifacts: Vec::new(),
            metadata: None,
        }
    }

    pub fn with_task_id(mut self, task_id: TaskId) -> Self {
        self.task_id = task_id;
        self
    }

    pub const fn with_state(mut self, state: TaskState) -> Self {
        self.state = state;
        self
    }

    pub fn with_response_text(mut self, text: String) -> Self {
        self.response_text = text;
        self
    }

    pub fn with_message_id(mut self, id: impl Into<MessageId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_user_message(mut self, message: Message) -> Self {
        self.user_message = Some(message);
        self
    }

    pub fn with_artifacts(mut self, artifacts: Vec<Artifact>) -> Self {
        self.artifacts = artifacts;
        self
    }

    pub fn with_metadata(mut self, metadata: TaskMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn build(self) -> Task {
        let agent_message = Message {
            role: "agent".to_string(),
            parts: vec![Part::Text(TextPart {
                text: self.response_text.clone(),
            })],
            id: self.id.clone(),
            task_id: Some(self.task_id.clone()),
            context_id: self.context_id.clone(),
            kind: "message".to_string(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

        let history = if let Some(user_msg) = self.user_message {
            Some(vec![
                user_msg,
                Message {
                    role: "agent".to_string(),
                    parts: vec![Part::Text(TextPart {
                        text: self.response_text.clone(),
                    })],
                    id: MessageId::generate(),
                    task_id: Some(self.task_id.clone()),
                    context_id: self.context_id.clone(),
                    kind: "message".to_string(),
                    metadata: None,
                    extensions: None,
                    reference_task_ids: None,
                },
            ])
        } else {
            None
        };

        Task {
            id: self.task_id.clone(),
            context_id: self.context_id.clone(),
            kind: "task".to_string(),
            status: TaskStatus {
                state: self.state,
                message: Some(agent_message),
                timestamp: Some(chrono::Utc::now()),
            },
            history,
            artifacts: if self.artifacts.is_empty() {
                None
            } else {
                Some(self.artifacts)
            },
            metadata: self.metadata,
        }
    }
}
