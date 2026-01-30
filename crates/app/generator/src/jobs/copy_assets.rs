use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_extension::{AssetDefinition, ExtensionRegistry};
use systemprompt_models::AppPaths;
use systemprompt_traits::JobResult;

pub async fn execute_copy_extension_assets() -> Result<JobResult> {
    let start_time = std::time::Instant::now();

    tracing::info!("Copy extension assets job started");

    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("AppPaths not initialized: {}", e))?;

    let registry = ExtensionRegistry::discover();
    let assets = registry.all_required_assets(paths);

    if assets.is_empty() {
        let duration_ms = start_time.elapsed().as_millis() as u64;
        tracing::info!(duration_ms, "No extension assets to copy");
        return Ok(JobResult::success()
            .with_stats(0, 0)
            .with_duration(duration_ms));
    }

    let dist_dir = paths.web().dist();

    let mut copied = 0u64;
    let mut failed = 0u64;

    for (ext_id, asset) in assets {
        match copy_asset(dist_dir, ext_id, &asset).await {
            Ok(()) => copied += 1,
            Err(e) => {
                if asset.is_required() {
                    return Err(e);
                }
                tracing::warn!(
                    extension = %ext_id,
                    asset = %asset.source().display(),
                    error = %e,
                    "Optional asset copy failed"
                );
                failed += 1;
            },
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;

    tracing::info!(
        copied,
        failed,
        duration_ms,
        "Copy extension assets job completed"
    );

    Ok(JobResult::success()
        .with_stats(copied, failed)
        .with_duration(duration_ms))
}

async fn copy_asset(dist_dir: &Path, ext_id: &str, asset: &AssetDefinition) -> Result<()> {
    let dest_path = dist_dir.join(asset.destination());

    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    tokio::fs::copy(asset.source(), &dest_path)
        .await
        .with_context(|| {
            format!(
                "Failed to copy asset from {} to {}",
                asset.source().display(),
                dest_path.display()
            )
        })?;

    tracing::debug!(
        extension = %ext_id,
        source = %asset.source().display(),
        destination = %dest_path.display(),
        "Copied extension asset"
    );

    Ok(())
}
