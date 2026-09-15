//! Nightly sweep of expired OAuth artifacts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use tracing::{debug, info};

use crate::repository::OauthCleanupRepository;

#[derive(Debug, Clone, Copy)]
pub struct OauthCleanupJob;

#[async_trait]
impl Job for OauthCleanupJob {
    fn name(&self) -> &'static str {
        "oauth_cleanup"
    }

    fn description(&self) -> &'static str {
        "Deletes expired OAuth refresh tokens, authorization codes, state bindings, JTI revocations and ID-JAG replay markers"
    }

    fn schedule(&self) -> &'static str {
        "0 5 3 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| {
            ProviderError::Configuration("DbPool not available in job context".into())
        })?);

        debug!("Job started");

        let repository = OauthCleanupRepository::new(&db_pool)
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;
        let counts = repository
            .delete_expired()
            .await
            .map_err(|e| ProviderError::Internal(e.to_string()))?;

        let duration_ms = u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);
        info!(
            oauth_codes = counts.codes,
            oauth_tokens = counts.tokens,
            oauth_state_bindings = counts.state_bindings,
            oauth_jti_revocations = counts.jti_revocations,
            id_jag_replays = counts.id_jag_replays,
            duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(counts.total(), 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&OauthCleanupJob);
