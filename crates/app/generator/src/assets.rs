use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn organize_css_files(web_dir: &str) -> Result<u32> {
    let dist_dir = PathBuf::from(web_dir);
    let css_dir = dist_dir.join("css");

    fs::create_dir_all(&css_dir)
        .await
        .context("Failed to create css directory")?;

    copy_files_by_extension(&dist_dir, &css_dir, "css").await
}

pub async fn organize_js_files(web_dir: &str) -> Result<u32> {
    let dist_dir = PathBuf::from(web_dir);
    let js_dir = dist_dir.join("js");

    fs::create_dir_all(&js_dir)
        .await
        .context("Failed to create js directory")?;

    Ok(0)
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

pub async fn copy_implementation_assets(_web_dir: &str) -> Result<u32> {
    Ok(0)
}
