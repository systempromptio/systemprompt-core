use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_core_content::ContentIngestionJob;
use systemprompt_core_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_traits::{Job, JobContext, JobResult};

use super::CopyExtensionAssetsJob;
use crate::{
    generate_sitemap, organize_css_files, organize_js_files, prerender_content, prerender_homepage,
};

#[derive(Debug, Clone, Copy)]
pub struct PublishContentJob;

struct PublishStats {
    succeeded: u64,
    failed: u64,
}

impl PublishStats {
    const fn new() -> Self {
        Self {
            succeeded: 0,
            failed: 0,
        }
    }

    fn record_success(&mut self) {
        self.succeeded += 1;
    }

    fn record_failure(&mut self) {
        self.failed += 1;
    }
}

impl PublishContentJob {
    pub async fn execute_publish(db_pool: &DbPool) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        tracing::info!("Publish content job started");

        let mut stats = PublishStats::new();

        run_content_ingestion(db_pool, &mut stats).await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        run_asset_copy(&mut stats).await;
        run_prerender(db_pool, &mut stats).await;
        run_homepage_prerender(db_pool, &mut stats).await;
        run_sitemap_generation(db_pool, &mut stats).await;
        run_css_organization(&mut stats).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            steps_succeeded = stats.succeeded,
            steps_failed = stats.failed,
            total_steps = stats.succeeded + stats.failed,
            duration_ms = duration_ms,
            "Publish content job completed"
        );

        Ok(JobResult::success()
            .with_stats(stats.succeeded, stats.failed)
            .with_duration(duration_ms))
    }
}

async fn run_content_ingestion(db_pool: &DbPool, stats: &mut PublishStats) {
    match ContentIngestionJob::execute_ingestion(db_pool).await {
        Ok(_) => stats.record_success(),
        Err(e) => {
            tracing::error!(error = %e, "Content ingestion failed");
            stats.record_failure();
        },
    }
}

async fn run_asset_copy(stats: &mut PublishStats) {
    match CopyExtensionAssetsJob::execute_copy().await {
        Ok(_) => {
            tracing::debug!("Extension asset copy completed");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "Extension asset copy failed");
            stats.record_failure();
        },
    }
}

async fn run_prerender(db_pool: &DbPool, stats: &mut PublishStats) {
    match prerender_content(Arc::clone(db_pool)).await {
        Ok(()) => {
            tracing::debug!("Prerendering completed");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!("Prerendering failed: {:#}", e);
            stats.record_failure();
        },
    }
}

async fn run_homepage_prerender(db_pool: &DbPool, stats: &mut PublishStats) {
    match prerender_homepage(Arc::clone(db_pool)).await {
        Ok(()) => {
            tracing::debug!("Homepage prerendering completed");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!("Homepage prerendering failed: {:#}", e);
            stats.record_failure();
        },
    }
}

async fn run_sitemap_generation(db_pool: &DbPool, stats: &mut PublishStats) {
    match generate_sitemap(Arc::clone(db_pool)).await {
        Ok(()) => {
            tracing::debug!("Sitemap generated");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "Sitemap generation warning");
            stats.record_failure();
        },
    }
}

async fn run_css_organization(stats: &mut PublishStats) {
    let web_dir = match AppPaths::get() {
        Ok(paths) => paths.web().dist(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get app paths");
            stats.record_failure();
            return;
        },
    };
    let Some(web_dir_str) = web_dir.to_str() else {
        tracing::warn!("Web dist path is not valid UTF-8");
        stats.record_failure();
        return;
    };

    match organize_css_files(web_dir_str).await {
        Ok(count) => {
            tracing::debug!(files = count, "CSS organized");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "CSS organization warning");
            stats.record_failure();
        },
    }

    match organize_js_files(web_dir_str).await {
        Ok(count) => {
            tracing::debug!(files = count, "JS organized");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "JS organization warning");
            stats.record_failure();
        },
    }
}

#[async_trait::async_trait]
impl Job for PublishContentJob {
    fn name(&self) -> &'static str {
        "publish_content"
    }

    fn description(&self) -> &'static str {
        "Publishes content through the full pipeline: images, ingestion, prerender, sitemap, CSS, \
         JS"
    }

    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Failed to get database pool from job context"))?;

        Self::execute_publish(pool).await
    }
}

systemprompt_traits::submit_job!(&PublishContentJob);
