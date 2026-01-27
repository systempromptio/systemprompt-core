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
    let dest_dir = dist_dir.join(ext);
    fs::create_dir_all(&dest_dir)
        .await
        .context(format!("Failed to create {} directory", ext))?;
    copy_files_by_extension(dist_dir, &dest_dir, ext).await
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

pub async fn copy_storage_assets_to_dist(
    storage: &StoragePaths,
    dist_dir: &Path,
) -> Result<(u32, u32)> {
    let mut css_count = 0;
    let mut js_count = 0;

    let css_source = storage.css();
    let css_dest = dist_dir.join("css");
    if css_source.exists() {
        fs::create_dir_all(&css_dest)
            .await
            .context("Failed to create css directory in dist")?;
        css_count = copy_directory_contents(css_source, &css_dest).await?;
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
        js_count = copy_directory_contents(js_source, &js_dest).await?;
    } else {
        tracing::info!(
            path = %js_source.display(),
            "Storage JS directory does not exist, skipping JS sync"
        );
    }

    Ok((css_count, js_count))
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

