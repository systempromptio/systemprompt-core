//! Composition helpers that construct single repositories for router state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::DynSessionUsageCounters;

pub fn session_usage(db: &DbPool) -> DynSessionUsageCounters {
    Arc::new(systemprompt_users::SessionRepository::new(db))
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
