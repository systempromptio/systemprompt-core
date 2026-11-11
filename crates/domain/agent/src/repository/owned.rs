//! Owner-scoped handles for user-owned tasks and artifacts.
//!
//! An [`OwnedContext`] is minted by `ContextRepository::owned_context`, and
//! the per-context listings take it.
//!
//! An [`OwnedTask`] or [`OwnedArtifact`] can only be minted by a repository
//! read whose SQL predicate carries both the entity id and the owning user
//! (`TaskRepository::owned_task`, `ArtifactRepository::owned_artifact`). The
//! user-facing reads and writes take the handle, so a caller holding only an
//! id has no path to another user's task, messages or artifacts.
//!
//! A [`RuntimeTask`] names the task the agent runtime is executing. It is
//! minted by `TaskRepository::create_task` when the runtime inserts the task
//! into a context the acting user owns, and by
//! `TaskRepository::run_task_in_tx`, which requires an agent run that names
//! the task, a task context owned by that run's user, and a [`RunClaim`]
//! matching the run row: the caller's lease, or no live lease at all. Task
//! state writes take a [`TaskHandle`] (either handle), so no write is keyed
//! on a bare id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
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

/// A context proven to belong to a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedContext {
    context: ContextId,
    user: UserId,
}

impl OwnedContext {
    pub(crate) const fn new(context_id: ContextId, user_id: UserId) -> Self {
        Self {
            context: context_id,
            user: user_id,
        }
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.context
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user
    }
}

/// A task the agent runtime created for the run it is executing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTask {
    task: TaskId,
    context: ContextId,
}

impl RuntimeTask {
    pub(crate) const fn new(task_id: TaskId, context_id: ContextId) -> Self {
        Self {
            task: task_id,
            context: context_id,
        }
    }

    pub const fn task_id(&self) -> &TaskId {
        &self.task
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.context
    }
}

/// The caller's claim on an agent run when minting its [`RuntimeTask`]: the
/// lease it holds, or `Unclaimed` for a run no worker holds (queued, released
/// or reclaimed in the same transaction).
#[derive(Debug, Clone, Copy)]
pub enum RunClaim<'a> {
    Lease {
        claimed_by: &'a str,
        attempt: i32,
        claimed_at: DateTime<Utc>,
    },
    Unclaimed,
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::OwnedTask {}
    impl Sealed for super::RuntimeTask {}
}

/// A task handle a state write accepts: an [`OwnedTask`] or a [`RuntimeTask`].
pub trait TaskHandle: sealed::Sealed + Sync {
    fn task_id(&self) -> &TaskId;
}

impl TaskHandle for OwnedTask {
    fn task_id(&self) -> &TaskId {
        &self.task
    }
}

impl TaskHandle for RuntimeTask {
    fn task_id(&self) -> &TaskId {
        &self.task
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
