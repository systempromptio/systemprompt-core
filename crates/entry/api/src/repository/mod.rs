//! Composition helpers that construct single repositories for router state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::DynSessionUsageCounters;

use crate::routes::webhook::WebhookRepositories;

pub fn context_provider(db: &DbPool) -> systemprompt_agent::services::ContextProviderService {
    systemprompt_agent::services::ContextProviderService::new(
        systemprompt_agent::repository::ContextRepository::new(db),
    )
}

pub fn session_usage(db: &DbPool) -> DynSessionUsageCounters {
    Arc::new(systemprompt_users::SessionRepository::new(db))
}

pub fn webhook(db: &DbPool, sessions: DynSessionUsageCounters) -> WebhookRepositories {
    WebhookRepositories {
        contexts: systemprompt_agent::repository::ContextRepository::new(db),
        tasks: systemprompt_agent::repository::task::TaskRepository::new(db, sessions),
        artifacts: systemprompt_agent::repository::content::ArtifactRepository::new(db),
        execution_steps: systemprompt_agent::repository::execution::ExecutionStepRepository::new(
            db,
        ),
    }
}

pub fn tool_usage(db: &DbPool) -> Arc<systemprompt_mcp::repository::ToolUsageRepository> {
    Arc::new(systemprompt_mcp::repository::ToolUsageRepository::new(db))
}

pub fn proxy_sessions(db: &DbPool) -> Arc<systemprompt_mcp::repository::McpProxySessionRepository> {
    Arc::new(systemprompt_mcp::repository::McpProxySessionRepository::new(db))
}

pub fn config_plane_state(
    db: &DbPool,
) -> Arc<systemprompt_scheduler::repository::ConfigReconcileRepository> {
    Arc::new(systemprompt_scheduler::repository::ConfigReconcileRepository::new(db))
}
