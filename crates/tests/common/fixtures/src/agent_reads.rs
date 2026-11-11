//! Owner-resolving reads over the agent repositories for test assertions.
//!
//! The agent repositories expose task and artifact reads and writes only
//! through owned handles. These helpers resolve the owner with a direct query
//! and go through that handle, so a test can read back or clean up what the
//! runtime persisted.

use std::future::Future;

use chrono::{DateTime, Utc};

use systemprompt_agent::models::a2a::{Artifact, Message, Task, TaskState};
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::repository::{ContextRepository, OwnedContext, OwnedTask};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_models::ExecutionStep;
use systemprompt_traits::RepositoryError;

async fn single_connection() -> Result<sqlx::PgConnection, RepositoryError> {
    use sqlx::Connection;
    sqlx::PgConnection::connect(&crate::test_database_url())
        .await
        .map_err(RepositoryError::database)
}

async fn task_owner(task_id: &TaskId) -> Result<Option<UserId>, RepositoryError> {
    let mut conn = single_connection().await?;
    let owner = sqlx::query_scalar!(
        r#"SELECT c.user_id as "user_id!: UserId"
        FROM agent_tasks t
        JOIN user_contexts c ON t.context_id = c.context_id
        WHERE t.task_id = $1"#,
        task_id.as_str()
    )
    .fetch_optional(&mut conn)
    .await
    .map_err(RepositoryError::database)?;
    Ok(owner)
}

async fn artifact_owner(artifact_id: &ArtifactId) -> Result<Option<UserId>, RepositoryError> {
    let mut conn = single_connection().await?;
    let owner = sqlx::query_scalar!(
        r#"SELECT c.user_id as "user_id!: UserId"
        FROM task_artifacts a
        JOIN agent_tasks t ON a.task_id = t.task_id
        JOIN user_contexts c ON t.context_id = c.context_id
        WHERE a.artifact_id = $1"#,
        artifact_id.as_str()
    )
    .fetch_optional(&mut conn)
    .await
    .map_err(RepositoryError::database)?;
    Ok(owner)
}

async fn owned_for(
    tasks: &TaskRepository,
    task_id: &TaskId,
) -> Result<Option<OwnedTask>, RepositoryError> {
    let Some(user_id) = task_owner(task_id).await? else {
        return Ok(None);
    };
    match tasks.owned_task(task_id, &user_id).await {
        Ok(owned) => Ok(Some(owned)),
        Err(e) if e.is_not_found() => Ok(None),
        Err(e) => Err(e),
    }
}

pub trait TaskReadsExt {
    fn read_task(
        &self,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<Option<Task>, RepositoryError>> + Send;

    fn read_task_messages(
        &self,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<Vec<Message>, RepositoryError>> + Send;

    fn owned_by_row(
        &self,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<Option<OwnedTask>, RepositoryError>> + Send;

    fn remove_task(
        &self,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn set_task_state(
        &self,
        task_id: &TaskId,
        state: TaskState,
        at: &DateTime<Utc>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    fn fail_task(
        &self,
        task_id: &TaskId,
        error: &str,
        at: &DateTime<Utc>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}

async fn require_owned(
    tasks: &TaskRepository,
    task_id: &TaskId,
) -> Result<OwnedTask, RepositoryError> {
    owned_for(tasks, task_id)
        .await?
        .ok_or_else(|| RepositoryError::not_found("task", task_id.as_str()))
}

impl TaskReadsExt for TaskRepository {
    async fn read_task(&self, task_id: &TaskId) -> Result<Option<Task>, RepositoryError> {
        match owned_for(self, task_id).await? {
            Some(owned) => self.find_owned_task(&owned).await.map(Some),
            None => Ok(None),
        }
    }

    async fn read_task_messages(&self, task_id: &TaskId) -> Result<Vec<Message>, RepositoryError> {
        match owned_for(self, task_id).await? {
            Some(owned) => self.owned_task_messages(&owned).await,
            None => Ok(Vec::new()),
        }
    }

    async fn owned_by_row(&self, task_id: &TaskId) -> Result<Option<OwnedTask>, RepositoryError> {
        owned_for(self, task_id).await
    }

    async fn remove_task(&self, task_id: &TaskId) -> Result<(), RepositoryError> {
        let owned = require_owned(self, task_id).await?;
        self.delete_task(&owned).await
    }

    async fn set_task_state(
        &self,
        task_id: &TaskId,
        state: TaskState,
        at: &DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let owned = require_owned(self, task_id).await?;
        self.update_task_state(&owned, state, at).await
    }

    async fn fail_task(
        &self,
        task_id: &TaskId,
        error: &str,
        at: &DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let owned = require_owned(self, task_id).await?;
        self.update_task_failed_with_error(&owned, error, at).await
    }
}

pub trait ArtifactReadsExt {
    fn read_artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> impl Future<Output = Result<Option<Artifact>, RepositoryError>> + Send;

    fn read_task_artifacts(
        &self,
        tasks: &TaskRepository,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<Vec<Artifact>, RepositoryError>> + Send;
}

impl ArtifactReadsExt for ArtifactRepository {
    async fn read_artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<Option<Artifact>, RepositoryError> {
        let Some(user_id) = artifact_owner(artifact_id).await? else {
            return Ok(None);
        };
        let owned = match self.owned_artifact(artifact_id, &user_id).await {
            Ok(owned) => owned,
            Err(e) if e.is_not_found() => return Ok(None),
            Err(e) => return Err(e),
        };
        self.find_owned_artifact(&owned).await.map(Some)
    }

    async fn read_task_artifacts(
        &self,
        tasks: &TaskRepository,
        task_id: &TaskId,
    ) -> Result<Vec<Artifact>, RepositoryError> {
        match owned_for(tasks, task_id).await? {
            Some(owned) => self.artifacts_for_owned_task(&owned).await,
            None => Ok(Vec::new()),
        }
    }
}

pub trait StepReadsExt {
    fn steps_for_task(
        &self,
        tasks: &TaskRepository,
        task_id: &TaskId,
    ) -> impl Future<Output = Result<Vec<ExecutionStep>, RepositoryError>> + Send;
}

impl StepReadsExt for ExecutionStepRepository {
    async fn steps_for_task(
        &self,
        tasks: &TaskRepository,
        task_id: &TaskId,
    ) -> Result<Vec<ExecutionStep>, RepositoryError> {
        match owned_for(tasks, task_id).await? {
            Some(owned) => self.list_by_task(&owned).await,
            None => Ok(Vec::new()),
        }
    }
}

pub trait ContextReadsExt {
    fn owned_by_row(
        &self,
        context_id: &ContextId,
    ) -> impl Future<Output = Result<OwnedContext, RepositoryError>> + Send;
}

impl ContextReadsExt for ContextRepository {
    async fn owned_by_row(&self, context_id: &ContextId) -> Result<OwnedContext, RepositoryError> {
        let mut conn = single_connection().await?;
        let owner = sqlx::query_scalar!(
            r#"SELECT user_id as "user_id!: UserId" FROM user_contexts WHERE context_id = $1"#,
            context_id.as_str()
        )
        .fetch_optional(&mut conn)
        .await
        .map_err(RepositoryError::database)?
        .ok_or_else(|| RepositoryError::not_found("context", context_id.as_str()))?;
        self.owned_context(context_id, &owner).await
    }
}
