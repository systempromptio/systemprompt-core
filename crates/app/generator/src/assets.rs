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

    copy_files_by_extension(&dist_dir, &js_dir, "js").await
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
    storage_dir: &Path,
    dist_dir: &Path,
) -> Result<(u32, u32)> {
    let mut css_count = 0;
    let mut js_count = 0;

    let css_source = storage_dir.join("files/css");
    let css_dest = dist_dir.join("css");
    if css_source.exists() {
        fs::create_dir_all(&css_dest)
            .await
            .context("Failed to create css directory in dist")?;
        css_count = copy_directory_contents(&css_source, &css_dest).await?;
    }

    let js_source = storage_dir.join("files/js");
    let js_dest = dist_dir.join("js");
    if js_source.exists() {
        fs::create_dir_all(&js_dest)
            .await
            .context("Failed to create js directory in dist")?;
        js_count = copy_directory_contents(&js_source, &js_dest).await?;
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

pub async fn copy_implementation_assets(_web_dir: &str) -> Result<u32> {
    Ok(0)
}
