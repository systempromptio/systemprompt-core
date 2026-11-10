//! Owner-scoped handles for user-owned tasks and artifacts.
//!
//! An [`OwnedTask`] or [`OwnedArtifact`] can only be minted by a repository
//! read whose SQL predicate carries both the entity id and the owning user
//! (`TaskRepository::owned_task`, `ArtifactRepository::owned_artifact`). The
//! user-facing reads and writes take the handle, so a caller holding only an
//! id has no path to another user's task, messages or artifacts. The agent
//! runtime's own id-keyed reads are crate-private.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};

/// A task proven to belong to a user's context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTask {
    task: TaskId,
    context: ContextId,
    user: UserId,
}

impl OwnedTask {
    pub(crate) const fn new(task_id: TaskId, context_id: ContextId, user_id: UserId) -> Self {
        Self {
            task: task_id,
            context: context_id,
            user: user_id,
        }
    }

    pub const fn task_id(&self) -> &TaskId {
        &self.task
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.context
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user
    }
}

/// An artifact proven to belong to a task in a user's context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedArtifact {
    artifact: ArtifactId,
    task: TaskId,
    context: ContextId,
    user: UserId,
}

impl OwnedArtifact {
    pub(crate) const fn new(
        artifact_id: ArtifactId,
        task_id: TaskId,
        context_id: ContextId,
        user_id: UserId,
    ) -> Self {
        Self {
            artifact: artifact_id,
            task: task_id,
            context: context_id,
            user: user_id,
        }
    }

    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact
    }

    pub const fn task_id(&self) -> &TaskId {
        &self.task
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.context
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user
    }
}
