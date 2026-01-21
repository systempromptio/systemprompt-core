use std::path::Path;
use tokio::fs;

use super::orchestrator::{BuildError, Result};

const CSS_FILES: &[&str] = &["content.css", "syntax-highlight.css"];

pub async fn organize_css(web_dir: &Path) -> Result<()> {
    tracing::debug!("Organizing CSS files");

    let dist_dir = web_dir.join("dist");
    let css_dir = dist_dir.join("css");

    fs::create_dir_all(&css_dir).await.map_err(|e| {
        BuildError::CssOrganizationFailed(format!("Failed to create css directory: {e}"))
    })?;

    for file_name in CSS_FILES {
        copy_css_file(&dist_dir, &css_dir, file_name).await?;
    }
    Ok(())
}

async fn copy_css_file(dist_dir: &Path, css_dir: &Path, file_name: &str) -> Result<()> {
    let source = dist_dir.join(file_name);
    if !source.exists() {
        tracing::warn!(file = %file_name, "CSS file not found, skipping");
        return Ok(());
    }
    do_copy_css(&source, &css_dir.join(file_name), file_name).await
}

async fn do_copy_css(source: &Path, dest: &Path, file_name: &str) -> Result<()> {
    fs::copy(source, dest).await.map_err(|e| {
        BuildError::CssOrganizationFailed(format!("Failed to copy {file_name} to css/: {e}"))
    })?;
    tracing::debug!(file = %file_name, "Copied CSS file to css/");
    Ok(())
}
