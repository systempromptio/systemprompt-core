//! Reaps `services` rows whose owning replica stopped heartbeating.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;

const DEFAULT_DEAD_AFTER_SECS: i64 = 90;

#[derive(Debug, Clone, Copy)]
pub struct ServiceRegistryGcJob;

#[async_trait]
impl Job for ServiceRegistryGcJob {
    fn name(&self) -> &'static str {
        "service_registry_gc"
    }

    fn description(&self) -> &'static str {
        "Removes service registry rows whose replica stopped heartbeating"
    }

    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| SchedulerError::missing_context("DbPool"))?;
        let dead_after_secs = ctx
            .get_parameter("dead_after_secs")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(DEFAULT_DEAD_AFTER_SECS);

        let repository = ServiceRepository::new(db_pool, InstanceId::new("scheduler"))
            .map_err(SchedulerError::from)?;
        let reaped = repository
            .delete_dead_instances(dead_after_secs)
            .await
            .map_err(SchedulerError::from)?;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        if reaped > 0 {
            info!(
                reaped,
                dead_after_secs, "service registry gc reaped dead instances"
            );
        }

        Ok(JobResult::success()
            .with_stats(0, reaped)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ServiceRegistryGcJob);
