//! On-demand job that backfills `country`/`region`/`city` on historical
//! sessions written before `GeoIP` was enabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_analytics::SessionRepository;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;

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

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );
        let app_context = Arc::clone(
            ctx.app_context::<Arc<AppContext>>()
                .ok_or_else(|| SchedulerError::missing_context("AppContext"))?,
        );

        let batch_size = ctx
            .get_parameter_parsed::<i64>("batch_size")?
            .unwrap_or(DEFAULT_BATCH_SIZE);

        let repository = SessionRepository::new(&db_pool).map_err(SchedulerError::from)?;
        let updated = if ctx.enforce() {
            repository
                .backfill_session_geo(app_context.geoip_reader(), batch_size)
                .await
                .map_err(SchedulerError::from)?
        } else {
            let candidates = repository
                .count_sessions_missing_geo()
                .await
                .map_err(SchedulerError::from)?;
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

systemprompt_provider_contracts::submit_job!(&BackfillSessionGeoJob);
