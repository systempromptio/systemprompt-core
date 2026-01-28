use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_content::ContentIngestionJob;
use systemprompt_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_traits::{Job, JobContext, JobResult};

use super::CopyExtensionAssetsJob;
use crate::{
    copy_storage_assets_to_dist, generate_feed, generate_sitemap, organize_dist_assets,
    prerender_content, prerender_homepage, warn_unexpected_dist_directories,
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
        run_storage_asset_copy(&mut stats).await;
        run_prerender(db_pool, &mut stats).await;
        run_homepage_prerender(db_pool, &mut stats).await;
        run_sitemap_generation(db_pool, &mut stats).await;
        run_rss_generation(db_pool, &mut stats).await;
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

async fn run_storage_asset_copy(stats: &mut PublishStats) {
    let paths = match AppPaths::get() {
        Ok(paths) => paths,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get app paths for storage asset copy");
            stats.record_failure();
            return;
        },
    };

    let storage = paths.storage();
    let dist_dir = paths.web().dist();

    warn_unexpected_dist_directories(dist_dir);

    match copy_storage_assets_to_dist(storage, dist_dir).await {
        Ok(asset_stats) => {
            tracing::info!(
                css = asset_stats.css,
                js = asset_stats.js,
                fonts = asset_stats.fonts,
                "Storage assets copied to dist"
            );
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "Storage asset copy failed");
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

async fn run_rss_generation(db_pool: &DbPool, stats: &mut PublishStats) {
    match generate_feed(Arc::clone(db_pool)).await {
        Ok(()) => {
            tracing::debug!("RSS feed generated");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "RSS feed generation warning");
            stats.record_failure();
        },
    }
}

async fn run_css_organization(stats: &mut PublishStats) {
    let dist_dir = match AppPaths::get() {
        Ok(paths) => paths.web().dist().to_path_buf(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get app paths");
            stats.record_failure();
            return;
        },
    };

    match organize_dist_assets(&dist_dir).await {
        Ok((css_count, js_count)) => {
            tracing::debug!(css = css_count, js = js_count, "Assets organized");
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "Asset organization failed");
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
        "Publishes content through the full pipeline: ingestion, prerender, sitemap, CSS, JS"
    }

    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"
    }

    fn run_on_startup(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Failed to get database pool from job context"))?;

        Self::execute_publish(pool).await
    }
}

systemprompt_provider_contracts::submit_job!(&PublishContentJob);
