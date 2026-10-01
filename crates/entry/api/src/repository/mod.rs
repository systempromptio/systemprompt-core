//! Composition modules that construct-and-store repository bundles and
//! single repositories for router state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod gateway;

pub use gateway::GatewayRepositories;

use std::sync::Arc;
use systemprompt_database::DbPool;

pub fn banned_ips(db: &DbPool) -> Arc<systemprompt_users::BannedIpRepository> {
    Arc::new(systemprompt_users::BannedIpRepository::new(db))
}

pub fn tool_usage(db: &DbPool) -> Arc<systemprompt_mcp::repository::ToolUsageRepository> {
    Arc::new(systemprompt_mcp::repository::ToolUsageRepository::new(db))
}

pub fn proxy_identities(
    db: &DbPool,
) -> Arc<systemprompt_mcp::repository::McpProxyIdentityRepository> {
    Arc::new(systemprompt_mcp::repository::McpProxyIdentityRepository::new(db))
}

pub fn user_rate_limit_buckets(
    db: &DbPool,
) -> Arc<systemprompt_users::UserRateLimitBucketRepository> {
    Arc::new(systemprompt_users::UserRateLimitBucketRepository::new(db))
}
