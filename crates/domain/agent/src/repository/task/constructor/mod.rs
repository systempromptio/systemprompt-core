//! Reassembly of persisted A2A [`Task`] aggregates from their relational rows.
//!
//! [`TaskConstructor`] fans out across the task, message, message-part,
//! artifact, and execution-step tables and rebuilds the nested [`Task`] graph,
//! offering both a single-task path and a batched path that amortises the
//! per-table round trips across many task ids.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod batch;
pub mod batch_builders;
pub(in crate::repository) mod batch_queries;
mod converters;
mod single;

use crate::models::a2a::Task;
use crate::repository::content::ArtifactRepository;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TaskId;
use systemprompt_traits::RepositoryError;

#[derive(Debug, Clone)]
pub struct TaskConstructor {
    pool: Arc<PgPool>,
    db_pool: DbPool,
    artifact_repo: ArtifactRepository,
}

impl TaskConstructor {
    pub fn new(db: &DbPool) -> Result<Self, crate::error::AgentError> {
        let pool = db
            .pool_arc()
            .map_err(|e| crate::error::AgentError::Init(e.to_string()))?;
        let artifact_repo = ArtifactRepository::new(db)?;
        Ok(Self {
            pool,
            db_pool: Arc::clone(db),
            artifact_repo,
        })
    }

    pub(crate) const fn pool(&self) -> &Arc<PgPool> {
        &self.pool
    }

    pub(crate) const fn artifact_repo(&self) -> &ArtifactRepository {
        &self.artifact_repo
    }

    pub(crate) const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    pub async fn construct_task_from_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<Task, RepositoryError> {
        single::construct_task_from_task_id(self, task_id).await
    }

    pub async fn construct_tasks_batch(
        &self,
        task_ids: &[TaskId],
    ) -> Result<Vec<Task>, RepositoryError> {
        batch::construct_tasks_batch(self, task_ids).await
    }
}
