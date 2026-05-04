//! Reorganise the freshly-built `dist/` directory by moving CSS and JS files
//! from the root into per-extension subdirectories so the generated site has
//! a tidy `dist/css/*.css` and `dist/js/*.js` layout.

use std::path::Path;
use tokio::fs;

use crate::error::{GeneratorResult, PublishError};

pub async fn organize_dist_assets(dist_dir: &Path) -> GeneratorResult<(u32, u32)> {
    let css_count = organize_assets_by_extension(dist_dir, "css").await?;
    let js_count = organize_assets_by_extension(dist_dir, "js").await?;
    Ok((css_count, js_count))
}

async fn organize_assets_by_extension(dist_dir: &Path, ext: &str) -> GeneratorResult<u32> {
    let target_dir = dist_dir.join(ext);
    fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| PublishError::other(format!("Failed to create {ext} directory: {e}")))?;
    copy_files_by_extension(dist_dir, &target_dir, ext).await
}

async fn copy_files_by_extension(
    source_dir: &Path,
    dest_dir: &Path,
    ext: &str,
) -> GeneratorResult<u32> {
    let mut copied = 0;
    let mut entries = fs::read_dir(source_dir)
        .await
        .map_err(|e| PublishError::other(format!("Failed to read source directory: {e}")))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| PublishError::other(format!("Failed to read entry: {e}")))?
    {
        let path = entry.path();
        let matches_ext = path.extension().is_some_and(|e| e == ext);

        if matches_ext {
            if let Some(file_name) = path.file_name() {
                let dest = dest_dir.join(file_name);
                fs::copy(&path, &dest).await.map_err(|e| {
                    PublishError::other(format!("Failed to copy {file_name:?}: {e}"))
                })?;
                copied += 1;
            }
        }
    }

    Ok(copied)
}
