//! Repository layer for the agent crate.
//!
//! Each submodule owns persistence for a domain aggregate (tasks, contexts,
//! artifacts, agent services, execution steps). The aggregate
//! [`A2ARepositories`] bundles them for callers that need the full A2A surface,
//! together with the two cross-domain read seams the agent runtime depends on:
//! the managed-skill resolver and the MCP tool-execution lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::{
    DynManagedSkillResolver, DynSessionUsageCounters, DynToolExecutionLookup,
};

pub mod agent_service;
pub mod content;
pub mod context;
pub mod execution;
pub mod ownership;
pub(crate) mod parts;
pub mod task;

pub use context::ContextRepository;
pub use ownership::AgentOwnerReassignment;
pub use systemprompt_traits::RepositoryError;

use crate::error::AgentError;

/// The collaborators [`A2ARepositories`] needs beyond the database: every
/// one is required, so a runtime without a managed-skill authority or a
/// tool-execution ledger cannot be assembled.
#[derive(Clone)]
pub struct A2aDependencies {
    pub session_usage: DynSessionUsageCounters,
    pub instance_id: InstanceId,
    pub managed_skills: DynManagedSkillResolver,
    pub tool_executions: DynToolExecutionLookup,
}

impl std::fmt::Debug for A2aDependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aDependencies")
            .field("instance_id", &self.instance_id)
            .field("managed_skills", &self.managed_skills)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct A2ARepositories {
    managed_skill_resolver: DynManagedSkillResolver,
    tool_executions: DynToolExecutionLookup,
    pub agent_services: agent_service::AgentServiceRepository,
    pub tasks: task::TaskRepository,
    pub contexts: ContextRepository,
    pub context_notifications: context::ContextNotificationRepository,
    pub artifacts: content::ArtifactRepository,
    pub execution_steps: execution::ExecutionStepRepository,
}

impl std::fmt::Debug for A2ARepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2ARepositories")
            .field("managed_skill_resolver", &self.managed_skill_resolver)
            .field("tasks", &self.tasks)
            .field("contexts", &self.contexts)
            .finish_non_exhaustive()
    }
}

impl A2ARepositories {
    pub fn new(db: &DbPool, deps: A2aDependencies) -> Result<Self, AgentError> {
        let A2aDependencies {
            session_usage,
            instance_id,
            managed_skills,
            tool_executions,
        } = deps;
        let agent_services = agent_service::AgentServiceRepository::new(db, instance_id)?;
        let tasks = task::TaskRepository::new(db, session_usage)?;
        let contexts = ContextRepository::new(db)?;
        let context_notifications = context::ContextNotificationRepository::new(db)
            .map_err(|e| AgentError::Init(e.to_string()))?;
        let artifacts = content::ArtifactRepository::new(db)?;
        let execution_steps = execution::ExecutionStepRepository::new(db)?;

        Ok(Self {
            managed_skill_resolver: managed_skills,
            tool_executions,
            agent_services,
            tasks,
            contexts,
            context_notifications,
            artifacts,
            execution_steps,
        })
    }

    #[must_use]
    pub fn with_managed_skill_resolver(mut self, resolver: DynManagedSkillResolver) -> Self {
        self.managed_skill_resolver = resolver;
        self
    }

    #[must_use]
    pub fn managed_skill_resolver(&self) -> DynManagedSkillResolver {
        Arc::clone(&self.managed_skill_resolver)
    }

    #[must_use]
    pub fn tool_executions(&self) -> DynToolExecutionLookup {
        Arc::clone(&self.tool_executions)
    }
}
