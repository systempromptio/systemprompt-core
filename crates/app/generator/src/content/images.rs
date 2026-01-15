use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_core_database::DbPool;
use systemprompt_core_files::FilesConfig;
use systemprompt_identifiers::ContentId;

use super::cards::normalize_image_url;
use crate::templates::{get_assets_path, load_web_config};
use systemprompt_core_content::repository::{ImageRepository, UnoptimizedImage};

pub async fn optimize_images(db_pool: DbPool) -> Result<()> {
    tracing::info!("Starting image optimization");

    let ctx = load_image_context(db_pool).await?;
    let rows = ctx.image_repo.find_unoptimized_images(100).await?;

    if rows.is_empty() {
        tracing::info!("No images require optimization");
        return Ok(());
    }

    tracing::debug!(count = rows.len(), "Found images to optimize");
    let stats = process_all_images(&rows, &ctx).await;
    log_optimization_stats(&stats);

    Ok(())
}

struct ImageContext {
    web_public: PathBuf,
    image_repo: ImageRepository,
}

struct OptimizationStats {
    optimized: u32,
    skipped: u32,
    errors: u32,
}

async fn load_image_context(db_pool: DbPool) -> Result<ImageContext> {
    let web_config = load_web_config()
        .await
        .context("Failed to load web config")?;
    let web_public = PathBuf::from(get_assets_path(&web_config)?);
    let image_repo = ImageRepository::new(&db_pool)?;
    Ok(ImageContext {
        web_public,
        image_repo,
    })
}

fn log_optimization_stats(stats: &OptimizationStats) {
    tracing::info!(
        optimized = stats.optimized,
        skipped = stats.skipped,
        errors = stats.errors,
        "Image optimization complete"
    );
}

async fn process_all_images(rows: &[UnoptimizedImage], ctx: &ImageContext) -> OptimizationStats {
    let mut stats = OptimizationStats {
        optimized: 0,
        skipped: 0,
        errors: 0,
    };

    for row in rows {
        process_image_row(row, ctx, &mut stats).await;
    }

    stats
}

async fn process_image_row(
    row: &UnoptimizedImage,
    ctx: &ImageContext,
    stats: &mut OptimizationStats,
) {
    let Some(image_path) = get_valid_image_path(row) else {
        return;
    };

    match process_single_image(&ctx.web_public, image_path, &ctx.image_repo, &row.id).await {
        Ok(true) => stats.optimized += 1,
        Ok(false) => stats.skipped += 1,
        Err(e) => {
            tracing::error!(image_path = %image_path, error = %e, "Failed to optimize");
            stats.errors += 1;
        },
    }
}

fn get_valid_image_path(row: &UnoptimizedImage) -> Option<&str> {
    let image_path = row.image.as_ref()?;

    if image_path.is_empty() {
        return None;
    }

    Some(image_path)
}

async fn process_single_image(
    web_public: &Path,
    image_url: &str,
    image_repo: &ImageRepository,
    content_id: &ContentId,
) -> Result<bool> {
    let normalized_url =
        normalize_image_url(Some(image_url)).unwrap_or_else(|| image_url.to_string());

    let source_path = resolve_image_path(web_public, image_url);
    let webp_path = resolve_image_path(web_public, &normalized_url);

    if !source_path.exists() {
        tracing::debug!(path = %source_path.display(), "Source not found");
        return Ok(false);
    }

    if webp_path.exists() {
        image_repo
            .update_image_url(content_id, &normalized_url)
            .await?;
        return Ok(false);
    }

    ensure_parent_dir(&webp_path)?;
    convert_to_webp(&source_path, &webp_path).await?;

    if !webp_path.exists() {
        anyhow::bail!("WebP not created: {}", webp_path.display());
    }

    image_repo
        .update_image_url(content_id, &normalized_url)
        .await?;

    Ok(true)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .context(format!("Failed to create dir: {}", parent.display()))?;
    }
    Ok(())
}

fn resolve_image_path(_web_public: &Path, image_url: &str) -> PathBuf {
    let Ok(files_config) = FilesConfig::get() else {
        return PathBuf::from(image_url.trim_start_matches('/'));
    };

    files_config
        .storage()
        .join(image_url.trim_start_matches('/'))
}

async fn convert_to_webp(source: &Path, dest: &Path) -> Result<()> {
    let source = source.to_path_buf();
    let dest = dest.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let img = image::open(&source)
            .with_context(|| format!("Failed to open image: {}", source.display()))?;

        img.save(&dest)
            .with_context(|| format!("Failed to save WebP: {}", dest.display()))?;

        Ok(())
    })
    .await
    .context("Image conversion task panicked")?
}
