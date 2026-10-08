//! Scheduled job that runs the registered page prerenderers (homepage,
//! search, error pages, …) shortly after the content prerender finishes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{
    Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult,
};

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

    fn scope(&self) -> JobScope {
        JobScope::Node
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = Arc::clone(ctx.get::<DbPool>()?);
        let paths = ctx.get::<Arc<AppPaths>>()?.as_ref();

        tracing::info!("Job started");
        let content_repo = ContentRepository::new(&db_pool);
        let results = prerender_pages(db_pool, content_repo, paths)
            .await
            .map_err(|e| ProviderError::Rendering {
                context: "page prerender".to_owned(),
                source: Box::new(e),
            })?;
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
