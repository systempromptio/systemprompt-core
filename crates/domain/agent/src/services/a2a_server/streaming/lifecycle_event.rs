//! The typed task-lifecycle notification posted to the internal broadcast
//! webhook.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};

use crate::models::a2a::Task;

/// A task-lifecycle notification for the internal broadcast webhook.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum LifecycleEvent {
    TaskCreated {
        entity_id: TaskId,
        context_id: ContextId,
        user_id: UserId,
        // JSON: the broadcast webhook wraps the task under `task_data.task`.
        task_data: serde_json::Value,
    },
    TaskCompleted {
        entity_id: TaskId,
        context_id: ContextId,
        user_id: UserId,
        // JSON: the broadcast webhook carries the serialised A2A task.
        task_data: serde_json::Value,
    },
    ArtifactCreated {
        entity_id: ArtifactId,
        context_id: ContextId,
        user_id: UserId,
    },
}

impl LifecycleEvent {
    pub fn task_created(task: &Task, user_id: &UserId) -> Result<Self, serde_json::Error> {
        Ok(Self::TaskCreated {
            entity_id: task.id.clone(),
            context_id: task.context_id.clone(),
            user_id: user_id.clone(),
            task_data: serde_json::json!({ "task": serde_json::to_value(task)? }),
        })
    }

    pub fn task_completed(task: &Task, user_id: &UserId) -> Result<Self, serde_json::Error> {
        Ok(Self::TaskCompleted {
            entity_id: task.id.clone(),
            context_id: task.context_id.clone(),
            user_id: user_id.clone(),
            task_data: serde_json::to_value(task)?,
        })
    }

    #[must_use]
    pub fn artifact_created(
        artifact_id: &ArtifactId,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Self {
        Self::ArtifactCreated {
            entity_id: artifact_id.clone(),
            context_id: context_id.clone(),
            user_id: user_id.clone(),
        }
    }
}
