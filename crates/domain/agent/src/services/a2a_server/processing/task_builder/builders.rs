//! Concrete [`Task`] builders for the completed and canceled paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TaskBuilder;
use crate::models::a2a::{Artifact, Message, Task, TaskState};
use systemprompt_identifiers::{ContextId, TaskId};

pub fn build_completed_task(
    task_id: TaskId,
    context_id: ContextId,
    response_text: String,
    user_message: Message,
    artifacts: Vec<Artifact>,
) -> Task {
    TaskBuilder::new(context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Completed)
        .with_response_text(response_text)
        .with_user_message(user_message)
        .with_artifacts(artifacts)
        .build()
}

pub fn build_canceled_task(task_id: TaskId, context_id: ContextId) -> Task {
    TaskBuilder::new(context_id)
        .with_task_id(task_id)
        .with_state(TaskState::Canceled)
        .with_response_text("Task was canceled.".to_owned())
        .build()
}
