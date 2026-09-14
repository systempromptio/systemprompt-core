//! Configured-owner inventory refresh runs at startup and on every node after
//! catalog changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::SchedulerError;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

#[derive(Debug, Clone, Copy)]
pub struct InventoryRefreshJob;

#[async_trait]
impl Job for InventoryRefreshJob {
    fn name(&self) -> &'static str {
        "managed_inventory_refresh"
    }
    fn description(&self) -> &'static str {
        "Reconciles configured and managed inventory with retained membership history"
    }
    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }
    fn scope(&self) -> JobScope {
        JobScope::Node
    }
    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let app = ctx
            .app_context::<Arc<AppContext>>()
            .ok_or_else(|| SchedulerError::missing_context("AppContext"))?;
        let status =
            systemprompt_runtime::optimization::inventory::refresh(app, &ctx.actor().user_id)
                .await
                .map_err(|error| SchedulerError::config_error(error.to_string()))?;
        Ok(JobResult::success().with_stats(
            u64::try_from(status.entries)
                .map_err(|error| SchedulerError::config_error(error.to_string()))?,
            0,
        ))
    }
}

systemprompt_provider_contracts::submit_job!(&InventoryRefreshJob);
