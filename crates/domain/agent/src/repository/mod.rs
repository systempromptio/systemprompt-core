//! Repository layer for the agent crate.
//!
//! Each submodule owns persistence for a domain aggregate (tasks, contexts,
//! artifacts, agent services, execution steps). The aggregate
//! [`A2ARepositories`] bundles them for callers that need the full A2A surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

#[derive(Debug, Clone)]
pub struct A2ARepositories {
    db_pool: DbPool,
    pub agent_services: agent_service::AgentServiceRepository,
    pub tasks: task::TaskRepository,
    pub contexts: ContextRepository,
    pub context_notifications: context::ContextNotificationRepository,
    pub artifacts: content::ArtifactRepository,
    pub execution_steps: execution::ExecutionStepRepository,
    pub push_notification_configs: content::PushNotificationConfigRepository,
}

impl A2ARepositories {
    pub fn new(
        db: &DbPool,
        session_usage: systemprompt_traits::DynSessionUsageCounters,
        instance_id: systemprompt_identifiers::InstanceId,
    ) -> Result<Self, AgentError> {
        let agent_services = agent_service::AgentServiceRepository::new(db, instance_id)?;
        let tasks = task::TaskRepository::new(db, session_usage)?;
        let contexts = ContextRepository::new(db)?;
        let context_notifications = context::ContextNotificationRepository::new(db)
            .map_err(|e| AgentError::Init(e.to_string()))?;
        let artifacts = content::ArtifactRepository::new(db)?;
        let execution_steps = execution::ExecutionStepRepository::new(db)?;
        let push_notification_configs = content::PushNotificationConfigRepository::new(db)?;

        Ok(Self {
            db_pool: Arc::clone(db),
            agent_services,
            tasks,
            contexts,
            context_notifications,
            artifacts,
            execution_steps,
            push_notification_configs,
        })
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }
}
