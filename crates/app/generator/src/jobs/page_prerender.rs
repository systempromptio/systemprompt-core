//! Scheduled job that runs the registered page prerenderers (homepage,
//! search, error pages, …) shortly after the content prerender finishes.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use crate::prerender::prerender_pages;

#[derive(Debug, Clone, Copy)]
pub struct PagePrerenderJob;

#[async_trait]
impl Job for PagePrerenderJob {
    fn name(&self) -> &'static str {
        "page_prerender"
    }

    fn description(&self) -> &'static str {
        "Prerenders all registered page prerenderers to static HTML"
    }

    fn schedule(&self) -> &'static str {
        "0 30 4 * * *"
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
        let results = prerender_pages(db_pool, paths)
            .await
            .map_err(|e| ProviderError::RenderFailed(e.to_string()))?;
        let pages_rendered = results.len() as u64;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            pages_rendered = pages_rendered,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(pages_rendered, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&PagePrerenderJob);
