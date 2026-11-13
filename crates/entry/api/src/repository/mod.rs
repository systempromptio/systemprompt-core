//! Composition helpers that construct single repositories for router state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;

pub fn tool_usage(db: &DbPool) -> Arc<systemprompt_mcp::repository::ToolUsageRepository> {
    Arc::new(systemprompt_mcp::repository::ToolUsageRepository::new(db))
}

pub fn proxy_sessions(db: &DbPool) -> Arc<systemprompt_mcp::repository::McpProxySessionRepository> {
    Arc::new(systemprompt_mcp::repository::McpProxySessionRepository::new(db))
}

pub fn user_rate_limit_buckets(
    db: &DbPool,
) -> Arc<systemprompt_users::UserRateLimitBucketRepository> {
    Arc::new(systemprompt_users::UserRateLimitBucketRepository::new(db))
}

pub fn config_plane_state(
    db: &DbPool,
) -> Arc<systemprompt_scheduler::repository::ConfigReconcileRepository> {
    Arc::new(systemprompt_scheduler::repository::ConfigReconcileRepository::new(db))
}
