//! Repository layer for the agent crate.
//!
//! Each submodule owns persistence for a domain aggregate (tasks, contexts,
//! artifacts, agent services, execution steps). The aggregate
//! [`A2ARepositories`] bundles them for callers that need the full A2A surface.

use std::sync::Arc;

use systemprompt_database::DbPool;

pub mod agent_service;
pub mod content;
pub mod context;
pub mod execution;
pub mod task;

pub use context::ContextRepository;
pub use systemprompt_traits::RepositoryError;

use crate::error::AgentError;

#[derive(Debug)]
pub struct A2ARepositories {
    db_pool: DbPool,
    pub agent_services: agent_service::AgentServiceRepository,
    pub tasks: task::TaskRepository,
    pub execution_steps: execution::ExecutionStepRepository,
    pub push_notification_configs: content::PushNotificationConfigRepository,
}

impl A2ARepositories {
    pub fn new(db: &DbPool) -> Result<Self, AgentError> {
        let agent_services = agent_service::AgentServiceRepository::new(db)?;
        let tasks = task::TaskRepository::new(db)?;
        let execution_steps = execution::ExecutionStepRepository::new(db)?;
        let push_notification_configs = content::PushNotificationConfigRepository::new(db)?;

        Ok(Self {
            db_pool: Arc::clone(db),
            agent_services,
            tasks,
            execution_steps,
            push_notification_configs,
        })
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }
}
