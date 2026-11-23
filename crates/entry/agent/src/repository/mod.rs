//! The repositories the agent route sets are built over, constructed once per
//! route set from the database handle and the session-usage counters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::repository::ContextRepository;
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::context::ContextNotificationRepository;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::DynSessionUsageCounters;

/// The conversation repositories the agent routes read and write.
#[derive(Clone)]
pub struct ConversationRepositories {
    pub contexts: ContextRepository,
    pub context_notifications: ContextNotificationRepository,
    pub tasks: TaskRepository,
    pub artifacts: ArtifactRepository,
    pub execution_steps: ExecutionStepRepository,
}

impl std::fmt::Debug for ConversationRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationRepositories")
            .finish_non_exhaustive()
    }
}

impl ConversationRepositories {
    #[must_use]
    pub fn new(db: &DbPool, sessions: DynSessionUsageCounters) -> Self {
        Self {
            contexts: ContextRepository::new(db),
            context_notifications: ContextNotificationRepository::new(db),
            tasks: TaskRepository::new(db, sessions),
            artifacts: ArtifactRepository::new(db),
            execution_steps: ExecutionStepRepository::new(db),
        }
    }
}


#[must_use]
pub fn service_table(db: &DbPool, instance: InstanceId) -> Arc<ServiceRepository> {
    Arc::new(ServiceRepository::new(db, instance))
}
