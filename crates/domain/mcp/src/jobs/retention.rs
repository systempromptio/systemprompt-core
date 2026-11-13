//! The MCP table `database_cleanup` prunes: `mcp_tool_executions` rows past
//! `retention.mcp_tool_executions_days`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;
use systemprompt_traits::{
    JobContext, ProviderError, ProviderResult, RetentionPurge, register_retention_purge,
};

use crate::repository::ToolUsageRepository;

struct ToolExecutionsPurge;

#[async_trait]
impl RetentionPurge for ToolExecutionsPurge {
    fn table(&self) -> &'static str {
        "mcp_tool_executions"
    }

    async fn purge_before(
        &self,
        ctx: &JobContext,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> ProviderResult<u64> {
        let db = Arc::clone(ctx.get::<DbPool>()?);
        ToolUsageRepository::new(&db)
            .purge_before(cutoff, batch)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))
    }
}

register_retention_purge!("mcp", &ToolExecutionsPurge);
