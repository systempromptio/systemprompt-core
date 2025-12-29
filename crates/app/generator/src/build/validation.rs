use quick_xml::de::from_str;
use serde::Deserialize;
use std::path::Path;
use tokio::fs;

use super::orchestrator::{BuildError, Result};

#[derive(Debug, Deserialize)]
struct Urlset {
    url: Vec<UrlEntry>,
}

#[derive(Debug, Deserialize)]
struct UrlEntry {
    loc: String,
}

#[derive(Debug)]
struct ValidationError {
    url: String,
    path: String,
    expected_file: String,
}

pub async fn validate_build(web_dir: &Path) -> Result<()> {
    let dist_dir = web_dir.join("dist");
    validate_required_paths(&dist_dir)?;
    validate_sitemap_if_exists(&dist_dir).await
}

fn validate_required_paths(dist_dir: &Path) -> Result<()> {
    if !dist_dir.exists() {
        return Err(BuildError::ValidationFailed(
            "dist directory not found".to_string(),
        ));
    }

    let index_html = dist_dir.join("index.html");
    if !index_html.exists() {
        return Err(BuildError::ValidationFailed(
            "index.html not found in dist".to_string(),
        ));
    }

    Ok(())
}

async fn validate_sitemap_if_exists(dist_dir: &Path) -> Result<()> {
    let sitemap_path = dist_dir.join("sitemap.xml");
    if !sitemap_path.exists() {
        tracing::warn!("sitemap.xml not found, skipping sitemap validation");
        return Ok(());
    }

    validate_sitemap(dist_dir, &sitemap_path).await
}

async fn validate_sitemap(dist_dir: &Path, sitemap_path: &Path) -> Result<()> {
    tracing::debug!("Validating sitemap.xml");

    let sitemap_xml = fs::read_to_string(sitemap_path)
        .await
        .map_err(|e| BuildError::ValidationFailed(format!("Failed to read sitemap: {e}")))?;

    let urlset: Urlset = from_str(&sitemap_xml)
        .map_err(|e| BuildError::ValidationFailed(format!("Failed to parse sitemap XML: {e}")))?;

    let (valid_count, missing_count, errors) = validate_urls(&urlset.url, dist_dir);
    check_validation_results(urlset.url.len(), valid_count, missing_count, &errors)?;

    Ok(())
}

fn validate_urls(urls: &[UrlEntry], dist_dir: &Path) -> (usize, usize, Vec<ValidationError>) {
    urls.iter()
        .filter_map(|entry| check_url_exists(entry, dist_dir))
        .fold(
            (0, 0, Vec::new()),
            |(valid, missing, mut errors), result| match result {
                Ok(()) => (valid + 1, missing, errors),
                Err(e) => {
                    errors.push(e);
                    (valid, missing + 1, errors)
                },
            },
        )
}

fn check_url_exists(
    entry: &UrlEntry,
    dist_dir: &Path,
) -> Option<std::result::Result<(), ValidationError>> {
    let path = extract_path_from_url(&entry.loc).ok()?;
    let html_path = resolve_html_path(dist_dir, &path);
    Some(check_html_exists(&entry.loc, &path, &html_path))
}

fn check_html_exists(
    url: &str,
    path: &str,
    html_path: &Path,
) -> std::result::Result<(), ValidationError> {
    if html_path.exists() {
        tracing::debug!(path = %path, "Valid URL");
        return Ok(());
    }

    tracing::warn!(path = %path, "Missing URL - no index.html found");
    Err(ValidationError {
        url: url.to_string(),
        path: path.to_string(),
        expected_file: html_path.display().to_string(),
    })
}

fn resolve_html_path(dist_dir: &Path, path: &str) -> std::path::PathBuf {
    if path == "/" {
        dist_dir.join("index.html")
    } else {
        dist_dir
            .join(path.trim_start_matches('/'))
            .join("index.html")
    }
}

fn check_validation_results(
    total: usize,
    valid: usize,
    missing: usize,
    errors: &[ValidationError],
) -> Result<()> {
    tracing::info!(
        total = total,
        valid = valid,
        missing = missing,
        "Sitemap validation summary"
    );

    if missing == 0 {
        tracing::info!("All sitemap URLs are valid");
        return Ok(());
    }

    log_validation_errors(errors);

    Err(BuildError::ValidationFailed(format!(
        "{missing} URLs missing corresponding HTML files"
    )))
}

fn log_validation_errors(errors: &[ValidationError]) {
    for error in errors {
        tracing::error!(
            url = %error.url,
            path = %error.path,
            expected = %error.expected_file,
            "Missing HTML file for sitemap URL"
        );
    }
}

fn extract_path_from_url(url: &str) -> Result<String> {
    if let Some(pos) = url.find("://") {
        if let Some(slash_pos) = url[pos + 3..].find('/') {
            return Ok(url[pos + 3 + slash_pos..].to_string());
        }
    }

    if url.starts_with('/') {
        return Ok(url.to_string());
    }

    Err(BuildError::ValidationFailed(format!(
        "Invalid URL format: {url}"
    )))
}
