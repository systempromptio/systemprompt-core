//! On-demand job that backfills `country`/`region`/`city` on historical
//! sessions written before `GeoIP` was enabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use tracing::info;

use crate::GeoIpReader;
use crate::repository::AnalyticsRepositories;

const DEFAULT_BATCH_SIZE: i64 = 1000;

#[derive(Debug, Clone, Copy)]
pub struct BackfillSessionGeoJob;

#[async_trait]
impl Job for BackfillSessionGeoJob {
    fn name(&self) -> &'static str {
        "backfill_session_geo"
    }

    fn description(&self) -> &'static str {
        "Backfills country/region/city on sessions with an IP but no geo data (parameter batch_size, default 1000); requires enforce"
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn schedulable(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let repositories = Arc::clone(ctx.get::<Arc<AnalyticsRepositories>>()?);
        let geoip = ctx.get::<GeoIpReader>().ok();

        let batch_size = ctx
            .get_parameter_parsed::<i64>("batch_size")?
            .unwrap_or(DEFAULT_BATCH_SIZE);

        let repository = &repositories.session_signals;
        let updated = if ctx.enforce() {
            repository
                .backfill_session_geo(geoip, batch_size)
                .await
                .map_err(|e| ProviderError::Internal(Box::new(e)))?
        } else {
            let candidates = repository
                .count_sessions_missing_geo()
                .await
                .map_err(|e| ProviderError::Internal(Box::new(e)))?;
            info!(
                candidate_sessions = candidates,
                "enforce disabled: sessions qualify for geo backfill but were not updated"
            );
            0
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            updated_sessions = updated,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(updated, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_traits::submit_job!(&BackfillSessionGeoJob);
