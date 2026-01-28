use anyhow::{Context, Result};
use std::path::Path;
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
