use anyhow::Result;
use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{Job, JobContext, JobResult};

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

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();
        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        tracing::info!("Job started");
        prerender_content(db_pool).await?;
        let duration_ms = start_time.elapsed().as_millis() as u64;
        tracing::info!(duration_ms = duration_ms, "Job completed");

        Ok(JobResult::success().with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ContentPrerenderJob);
