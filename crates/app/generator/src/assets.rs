use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn organize_css_files(web_dir: &str) -> Result<u32> {
    let dist_dir = PathBuf::from(web_dir);
    let css_dir = dist_dir.join("css");

    fs::create_dir_all(&css_dir)
        .await
        .context("Failed to create css directory")?;

    let mut copied = 0;

    copied += copy_files_by_extension(&dist_dir, &css_dir, "css").await?;

    if let Ok(impl_assets_path) = std::env::var("SYSTEMPROMPT_WEB_ASSETS_PATH") {
        let impl_css_dir = PathBuf::from(&impl_assets_path).join("css");
        if path_exists(&impl_css_dir).await {
            copied += copy_files_by_extension(&impl_css_dir, &css_dir, "css").await?;
        }
    }

    Ok(copied)
}

pub async fn organize_js_files(web_dir: &str) -> Result<u32> {
    let dist_dir = PathBuf::from(web_dir);
    let js_dir = dist_dir.join("js");

    fs::create_dir_all(&js_dir)
        .await
        .context("Failed to create js directory")?;

    let mut copied = 0;

    if let Ok(impl_assets_path) = std::env::var("SYSTEMPROMPT_WEB_ASSETS_PATH") {
        let impl_js_dir = PathBuf::from(&impl_assets_path).join("js");
        if path_exists(&impl_js_dir).await {
            copied += copy_files_by_extension(&impl_js_dir, &js_dir, "js").await?;
        }
    }

    Ok(copied)
}

async fn path_exists(path: &Path) -> bool {
    match fs::try_exists(path).await {
        Ok(exists) => exists,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "Failed to check path existence");
            false
        },
    }
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

pub async fn copy_implementation_assets(web_dir: &str) -> Result<u32> {
    let Ok(impl_assets_path) = std::env::var("SYSTEMPROMPT_WEB_ASSETS_PATH") else {
        return Ok(0);
    };

    let impl_assets = PathBuf::from(&impl_assets_path);
    if !path_exists(&impl_assets).await {
        return Ok(0);
    }

    let web_dir_path = PathBuf::from(web_dir);
    let core_assets = web_dir_path.join("../src/assets");
    let public_dir = web_dir_path.join("../public");

    let asset_types = vec!["fonts", "logos", "images"];
    let mut copied_count = 0;

    for asset_type in asset_types {
        let src_dir = impl_assets.join(asset_type);
        if !path_exists(&src_dir).await {
            continue;
        }

        let dest_dir = core_assets.join(asset_type);
        copy_directory_recursive(&src_dir, &dest_dir).await?;

        if asset_type == "fonts" {
            let public_dest = public_dir.join(asset_type);
            copy_directory_recursive(&src_dir, &public_dest).await?;
        }
        copied_count += 1;
    }

    let static_files = vec![
        ("favicon.ico", public_dir.join("favicon.ico")),
        ("robots.txt", public_dir.join("robots.txt")),
        ("llms.txt", public_dir.join("llms.txt")),
    ];

    for (file_name, dest_path) in static_files {
        let src_path = impl_assets.join(file_name);
        if path_exists(&src_path).await {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create parent directory")?;
            }
            fs::copy(&src_path, &dest_path)
                .await
                .context(format!("Failed to copy {file_name}"))?;
            copied_count += 1;
        }
    }

    Ok(copied_count)
}

async fn copy_directory_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)
        .await
        .context(format!("Failed to create directory: {}", dest.display()))?;

    let mut entries = fs::read_dir(src)
        .await
        .context(format!("Failed to read directory: {}", src.display()))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Failed to read directory entry")?
    {
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        let metadata = entry
            .metadata()
            .await
            .context("Failed to get entry metadata")?;

        if metadata.is_dir() {
            Box::pin(copy_directory_recursive(&src_path, &dest_path)).await?;
        } else {
            fs::copy(&src_path, &dest_path)
                .await
                .context(format!("Failed to copy file: {}", src_path.display()))?;
        }
    }

    Ok(())
}
