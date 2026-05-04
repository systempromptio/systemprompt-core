//! Scheduled job that runs the content prerender pipeline once a day.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use crate::prerender::prerender_content;

/// Scheduled job that prerenders every configured content source.
#[derive(Debug, Clone, Copy)]
pub struct ContentPrerenderJob;

#[async_trait]
impl Job for ContentPrerenderJob {
    fn name(&self) -> &'static str {
        "content_prerender"
    }

    fn description(&self) -> &'static str {
        "Prerenders all configured content sources to static HTML"
    }

    fn schedule(&self) -> &'static str {
        "0 0 4 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| {
            ProviderError::Configuration("DbPool not available in job context".into())
        })?);
        let paths = ctx
            .app_paths::<Arc<AppPaths>>()
            .ok_or_else(|| {
                ProviderError::Configuration("AppPaths not available in job context".into())
            })?
            .as_ref();

        tracing::info!("Job started");
        prerender_content(db_pool, paths)
            .await
            .map_err(|e| ProviderError::RenderFailed(e.to_string()))?;
        let duration_ms = start_time.elapsed().as_millis() as u64;
        tracing::info!(duration_ms = duration_ms, "Job completed");

        Ok(JobResult::success().with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ContentPrerenderJob);
