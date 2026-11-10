//! Owner-resolving reads over the agent repositories for test assertions.
//!
//! The agent repositories expose task and artifact reads only through owned
//! handles. These helpers resolve the owner from the stored row and read
//! through that handle, so a test can read back what the runtime persisted.

use std::future::Future;

use systemprompt_agent::models::a2a::{Artifact, Message, Task};
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::repository::OwnedTask;
use systemprompt_identifiers::{ArtifactId, TaskId};
use systemprompt_traits::RepositoryError;

async fn owned_for(
    tasks: &TaskRepository,
    task_id: &TaskId,
) -> Result<Option<OwnedTask>, RepositoryError> {
    let Some(info) = tasks.find_task_context_info(task_id).await? else {
        return Ok(None);
    };
    let Some(user_id) = info.user_id else {
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
        let all = self.get_all_artifacts(Some(i32::MAX)).await?;
        Ok(all.into_iter().find(|a| a.id == *artifact_id))
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
