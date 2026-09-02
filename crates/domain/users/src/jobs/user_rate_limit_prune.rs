//! Scheduled job pruning elapsed `user_rate_limit_buckets` windows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::ProviderError;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::UserRateLimitBucketRepository;

const DEFAULT_RETAIN_SECS: i64 = 3600;

#[derive(Debug, Clone, Copy)]
pub struct UserRateLimitPruneJob;

#[async_trait]
impl Job for UserRateLimitPruneJob {
    fn name(&self) -> &'static str {
        "user_rate_limit_prune"
    }

    fn description(&self) -> &'static str {
        "Deletes rate-limit windows older than retain_secs (default 3600)"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| {
            ProviderError::Configuration("DbPool not available in job context".into())
        })?);

        info!("Job started");

        let retain_secs = ctx
            .get_parameter_parsed::<i64>("retain_secs")?
            .unwrap_or(DEFAULT_RETAIN_SECS);
        let before = Utc::now() - Duration::seconds(retain_secs);

        let repository = UserRateLimitBucketRepository::new(&db_pool)
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;
        let pruned = repository
            .prune(before)
            .await
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            pruned_windows = pruned,
            retain_secs = retain_secs,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(pruned, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&UserRateLimitPruneJob);
