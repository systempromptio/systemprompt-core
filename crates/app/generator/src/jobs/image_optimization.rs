use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};

use crate::optimize_images;

#[derive(Debug, Clone, Copy)]
pub struct ImageOptimizationJob;

impl ImageOptimizationJob {
    pub async fn execute_optimization(db_pool: &DbPool) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        tracing::info!("Image optimization job started");

        match optimize_images(Arc::clone(db_pool)).await {
            Ok(()) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                tracing::info!(duration_ms, "Image optimization job completed");
                Ok(JobResult::success()
                    .with_stats(1, 0)
                    .with_duration(duration_ms))
            },
            Err(e) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                tracing::error!(error = %e, duration_ms, "Image optimization job failed");
                Ok(JobResult::failure(e.to_string())
                    .with_stats(0, 1)
                    .with_duration(duration_ms))
            },
        }
    }
}

#[async_trait::async_trait]
impl Job for ImageOptimizationJob {
    fn name(&self) -> &'static str {
        "image_optimization"
    }

    fn description(&self) -> &'static str {
        "Converts images to WebP format for optimization"
    }

    fn schedule(&self) -> &'static str {
        "0 */30 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Failed to get database pool from job context"))?;

        Self::execute_optimization(pool).await
    }
}

systemprompt_traits::submit_job!(&ImageOptimizationJob);
