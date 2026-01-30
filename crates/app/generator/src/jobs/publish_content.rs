use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_content::execute_content_ingestion;
use systemprompt_database::{DatabaseProvider, DbPool};
use systemprompt_models::{AppPaths, ProfileBootstrap};
use systemprompt_sync::PlaybooksLocalSync;
use systemprompt_traits::{Job, JobContext, JobResult};

use super::execute_copy_extension_assets;
use crate::{
    generate_feed, generate_sitemap, organize_dist_assets, prerender_content, prerender_pages,
};

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

pub async fn execute_publish_content(db_pool: &DbPool) -> Result<JobResult> {
    let start_time = std::time::Instant::now();

    tracing::info!("Publish content job started");

    let mut stats = PublishStats::new();

    run_content_ingestion(db_pool, &mut stats).await;
    run_playbook_sync(db_pool, &mut stats).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    run_asset_copy(&mut stats).await;
    run_prerender(db_pool, &mut stats).await;
    run_page_prerender(db_pool, &mut stats).await;
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

async fn run_content_ingestion(db_pool: &DbPool, stats: &mut PublishStats) {
    match execute_content_ingestion(db_pool).await {
        Ok(_) => stats.record_success(),
        Err(e) => {
            tracing::error!(error = %e, "Content ingestion failed");
            stats.record_failure();
        },
    }
}

async fn run_asset_copy(stats: &mut PublishStats) {
    match execute_copy_extension_assets().await {
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

async fn run_playbook_sync(db_pool: &DbPool, stats: &mut PublishStats) {
    let Some(playbooks_path) = get_playbooks_path() else {
        tracing::debug!("Playbooks path not configured or does not exist, skipping sync");
        return;
    };

    #[allow(clippy::clone_on_ref_ptr)]
    let db: Arc<dyn DatabaseProvider> = db_pool.clone();
    let sync = PlaybooksLocalSync::new(db, playbooks_path);

    let diff = match sync.calculate_diff().await {
        Ok(diff) => diff,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to calculate playbooks diff");
            stats.record_failure();
            return;
        },
    };

    if !diff.has_changes() {
        tracing::debug!("Playbooks are in sync, no changes needed");
        stats.record_success();
        return;
    }

    match sync.sync_to_db(&diff, false).await {
        Ok(result) => {
            tracing::info!(
                direction = %result.direction,
                synced = result.items_synced,
                skipped = result.items_skipped,
                "Playbooks synced to database"
            );
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!(error = %e, "Playbook sync failed");
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

async fn run_page_prerender(db_pool: &DbPool, stats: &mut PublishStats) {
    match prerender_pages(Arc::clone(db_pool)).await {
        Ok(results) => {
            let page_count = results.len();
            if page_count > 0 {
                tracing::debug!(page_count = page_count, "Page prerendering completed");
            } else {
                tracing::debug!("No page prerenderers registered");
            }
            stats.record_success();
        },
        Err(e) => {
            tracing::warn!("Page prerendering failed: {:#}", e);
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

fn get_playbooks_path() -> Option<PathBuf> {
    let profile = ProfileBootstrap::get().ok()?;
    let playbooks_path = PathBuf::from(format!("{}/playbook", profile.paths.services));
    if playbooks_path.exists() {
        Some(playbooks_path)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContentPublishJob;

#[async_trait]
impl Job for ContentPublishJob {
    fn name(&self) -> &'static str {
        "content_publish"
    }

    fn description(&self) -> &'static str {
        "Full content publishing pipeline: ingest, assets, prerender, sitemap, RSS"
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn tags(&self) -> Vec<&'static str> {
        vec!["content", "publish", "prerender"]
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let db_pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?;

        execute_publish_content(db_pool).await
    }
}

systemprompt_provider_contracts::submit_job!(&ContentPublishJob);
