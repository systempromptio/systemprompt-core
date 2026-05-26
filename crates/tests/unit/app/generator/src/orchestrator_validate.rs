//! Exercise `BuildOrchestrator::validate_only` against temp-dir layouts so
//! the validation pipeline (validate_required_paths, validate_sitemap,
//! resolve_html_path, extract_path_from_url, check_validation_results) is
//! actually executed.

use std::fs;
use systemprompt_generator::{BuildMode, BuildOrchestrator};
use tempfile::TempDir;

fn make_orchestrator(tmp: &TempDir) -> BuildOrchestrator {
    BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production)
}

#[tokio::test]
async fn validate_only_missing_dist_returns_validation_failed() {
    let tmp = TempDir::new().unwrap();
    let orch = make_orchestrator(&tmp);
    let err = orch.validate_only().await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Validation failed"));
    assert!(msg.contains("dist"));
}

#[tokio::test]
async fn validate_only_missing_index_returns_validation_failed() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("dist")).unwrap();
    let orch = make_orchestrator(&tmp);
    let err = orch.validate_only().await.unwrap_err();
    assert!(err.to_string().contains("index.html"));
}

#[tokio::test]
async fn validate_only_passes_without_sitemap() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.validate_only().await.unwrap();
}

#[tokio::test]
async fn validate_only_passes_with_valid_sitemap_root_url() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/</loc></url>
</urlset>"#,
    )
    .unwrap();
    let orch = make_orchestrator(&tmp);
    orch.validate_only().await.unwrap();
}

#[tokio::test]
async fn validate_only_fails_when_sitemap_url_missing_html() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/missing/</loc></url>
</urlset>"#,
    )
    .unwrap();
    let orch = make_orchestrator(&tmp);
    let err = orch.validate_only().await.unwrap_err();
    assert!(err.to_string().contains("missing"));
}

#[tokio::test]
async fn validate_only_handles_nested_url_with_existing_page() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    let blog_post = dist.join("blog").join("hello");
    fs::create_dir_all(&blog_post).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(blog_post.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/blog/hello</loc></url>
</urlset>"#,
    )
    .unwrap();
    let orch = make_orchestrator(&tmp);
    orch.validate_only().await.unwrap();
}

#[tokio::test]
async fn validate_only_invalid_sitemap_xml_errors() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("sitemap.xml"), b"not xml").unwrap();
    let orch = make_orchestrator(&tmp);
    let err = orch.validate_only().await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Validation failed"));
}

#[tokio::test]
async fn validate_only_relative_loc_treated_as_path() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>/</loc></url>
</urlset>"#,
    )
    .unwrap();
    let orch = make_orchestrator(&tmp);
    orch.validate_only().await.unwrap();
}

#[test]
fn mode_accessor_returns_configured_mode() {
    let tmp = TempDir::new().unwrap();
    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Docker);
    assert_eq!(orch.mode(), BuildMode::Docker);
}
