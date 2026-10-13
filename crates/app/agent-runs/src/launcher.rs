//! [`RepositoryRunLauncher`]: the `AgentRunLauncher` an A2A server queues
//! task-backed runs through.
//!
//! `enqueue` decodes the opaque request as a [`RunRequest`] and inserts a
//! queued run carrying the task and context; the run's claim loop takes it
//! from there. `cancel` records the request for a running run and cancels a
//! still-queued one outright, together with its task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentRunId, TaskId};
use systemprompt_traits::{
    AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher, DynAgentRunLauncher,
};

use crate::repository::{AgentRunRepository, NewAgentRun};
use crate::request::RunRequest;

#[derive(Debug, Clone)]
pub struct RepositoryRunLauncher {
    repo: AgentRunRepository,
}

impl RepositoryRunLauncher {
    #[must_use]
    pub const fn new(repo: AgentRunRepository) -> Self {
        Self { repo }
    }

    #[must_use]
    pub fn shared(db: &DbPool) -> DynAgentRunLauncher {
        Arc::new(Self::new(AgentRunRepository::new(db)))
    }
}

#[async_trait]
impl AgentRunLauncher for RepositoryRunLauncher {
    async fn enqueue(&self, launch: AgentRunLaunch) -> Result<AgentRunId, AgentRunLaunchError> {
        let request: RunRequest =
            serde_json::from_value(launch.request).map_err(AgentRunLaunchError::InvalidRequest)?;
        let id = AgentRunId::generate();
        self.repo
            .insert_run(NewAgentRun {
                id: &id,
                origin_kind: &launch.origin_kind,
                origin_id: &launch.origin_id,
                user_id: &launch.user_id,
                request: &request,
                not_before: None,
                task_id: Some(&launch.task_id),
                context_id: Some(&launch.context_id),
            })
            .await
            .map_err(AgentRunLaunchError::unavailable)?;
        Ok(id)
    }

    async fn cancel(&self, run_id: &AgentRunId) -> Result<(), AgentRunLaunchError> {
        self.repo
            .update_cancel_requested(run_id)
            .await
            .map_err(AgentRunLaunchError::unavailable)?;
        self.repo
            .cancel_queued(run_id)
            .await
            .map_err(AgentRunLaunchError::unavailable)?;
        Ok(())
    }

    async fn run_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<AgentRunId>, AgentRunLaunchError> {
        self.repo
            .find_run_for_task(task_id)
            .await
            .map_err(AgentRunLaunchError::unavailable)
    }
}
