use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_models::StoragePaths;
use tokio::fs;

pub async fn organize_dist_assets(dist_dir: &Path) -> Result<(u32, u32)> {
    let css_count = organize_assets_by_extension(dist_dir, "css").await?;
    let js_count = organize_assets_by_extension(dist_dir, "js").await?;
    Ok((css_count, js_count))
}

async fn organize_assets_by_extension(dist_dir: &Path, ext: &str) -> Result<u32> {
    let target_dir = dist_dir.join(ext);
    fs::create_dir_all(&target_dir)
        .await
        .context(format!("Failed to create {} directory", ext))?;
    copy_files_by_extension(dist_dir, &target_dir, ext).await
}

async fn copy_files_by_extension(source_dir: &Path, dest_dir: &Path, ext: &str) -> Result<u32> {
    let mut copied = 0;
    let mut entries = fs::read_dir(source_dir)
        .await
        .context("Failed to read source directory")?;

    while let Some(entry) = entries.next_entry().await.context("Failed to read entry")? {
        let path = entry.path();
        let matches_ext = path.extension().is_some_and(|e| e == ext);

        if matches_ext {
            if let Some(file_name) = path.file_name() {
                let dest = dest_dir.join(file_name);
                fs::copy(&path, &dest)
                    .await
                    .context(format!("Failed to copy {file_name:?}"))?;
                copied += 1;
            }
        }
    }

    Ok(copied)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AssetCopyStats {
    pub css: u32,
    pub js: u32,
    pub fonts: u32,
}

pub async fn copy_storage_assets_to_dist(
    storage: &StoragePaths,
    dist_dir: &Path,
) -> Result<AssetCopyStats> {
    let mut stats = AssetCopyStats {
        css: 0,
        js: 0,
        fonts: 0,
    };

    let css_source = storage.css();
    let css_dest = dist_dir.join("css");
    if css_source.exists() {
        fs::create_dir_all(&css_dest)
            .await
            .context("Failed to create css directory in dist")?;
        stats.css = copy_directory_contents(css_source, &css_dest).await?;
    } else {
        tracing::info!(
            path = %css_source.display(),
            "Storage CSS directory does not exist, skipping CSS sync"
        );
    }

    let js_source = storage.js();
    let js_dest = dist_dir.join("js");
    if js_source.exists() {
        fs::create_dir_all(&js_dest)
            .await
            .context("Failed to create js directory in dist")?;
        stats.js = copy_directory_contents(js_source, &js_dest).await?;
    } else {
        tracing::info!(
            path = %js_source.display(),
            "Storage JS directory does not exist, skipping JS sync"
        );
    }

    let fonts_source = storage.fonts();
    let fonts_dest = dist_dir.join("fonts");
    if fonts_source.exists() {
        fs::create_dir_all(&fonts_dest)
            .await
            .context("Failed to create fonts directory in dist")?;
        stats.fonts = copy_directory_contents(fonts_source, &fonts_dest).await?;
    } else {
        tracing::info!(
            path = %fonts_source.display(),
            "Storage fonts directory does not exist, skipping fonts sync"
        );
    }

    Ok(stats)
}

pub fn warn_unexpected_dist_directories(dist_dir: &Path) {
    let unexpected = ["files/css", "files/js"];
    for dir in unexpected {
        let path = dist_dir.join(dir);
        if path.exists() {
            tracing::warn!(
                path = %path.display(),
                "Unexpected directory found in dist - CSS/JS should be in dist/css/ and dist/js/, not dist/files/"
            );
        }
    }
}

async fn copy_directory_contents(source: &Path, dest: &Path) -> Result<u32> {
    let mut copied = 0;
    let mut entries = fs::read_dir(source)
        .await
        .context("Failed to read source directory")?;

    while let Some(entry) = entries.next_entry().await.context("Failed to read entry")? {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let dest_path = dest.join(file_name);
                fs::copy(&path, &dest_path)
                    .await
                    .context(format!("Failed to copy {:?}", file_name))?;
                copied += 1;
            }
        }
    }

    Ok(copied)
}
