//! Scheduled job that runs the content prerender pipeline once a day.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_config::paths::AppPaths;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{
    Job, JobContext, JobResult, JobScope, ProviderError, ProviderResult,
};

use crate::prerender::prerender_content;

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

    fn scope(&self) -> JobScope {
        JobScope::Node
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = Arc::clone(ctx.get::<DbPool>()?);
        let paths = ctx.get::<Arc<AppPaths>>()?.as_ref();

        tracing::info!("Job started");
        let content_repo = ContentRepository::new(&db_pool);
        let content_analytics = ContentAnalyticsRepository::new(&db_pool);
        prerender_content(db_pool, content_repo, content_analytics, paths)
            .await
            .map_err(|e| ProviderError::Rendering {
                context: "content prerender".to_owned(),
                source: Box::new(e),
            })?;
        let duration_ms = start_time.elapsed().as_millis() as u64;
        tracing::info!(duration_ms = duration_ms, "Job completed");

        Ok(JobResult::success().with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ContentPrerenderJob);
