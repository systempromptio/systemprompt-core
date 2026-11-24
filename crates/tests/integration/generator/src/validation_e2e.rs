//! Exercises the validation half of the build pipeline: `validate_only()`
//! against scaffolds with and without `sitemap.xml`, covering valid URL
//! resolution, missing URL detection, and unparseable URL handling.

use std::path::Path;
use systemprompt_generator::{BuildMode, BuildOrchestrator};
use tempfile::TempDir;
use tokio::fs;

async fn write_index(dist: &Path) {
    fs::create_dir_all(dist).await.expect("mkdir dist");
    fs::write(dist.join("index.html"), "<html></html>")
        .await
        .expect("write index.html");
}

async fn write_sitemap(dist: &Path, body: &str) {
    fs::write(dist.join("sitemap.xml"), body)
        .await
        .expect("write sitemap.xml");
}

#[tokio::test]
async fn validate_only_passes_when_no_sitemap_present() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    orchestrator
        .validate_only()
        .await
        .expect("validate_only must succeed with just index.html");
}

#[tokio::test]
async fn validate_only_fails_when_dist_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    let err = orchestrator
        .validate_only()
        .await
        .expect_err("must fail with no dist");
    assert!(err.to_string().to_lowercase().contains("dist"));
}

#[tokio::test]
async fn validate_only_accepts_sitemap_with_resolvable_urls() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    let about_dir = dist.join("about");
    fs::create_dir_all(&about_dir).await.expect("mkdir about");
    fs::write(about_dir.join("index.html"), "<html>about</html>")
        .await
        .expect("write about/index.html");

    let sitemap = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>/</loc></url>
  <url><loc>/about</loc></url>
  <url><loc>https://example.com/about</loc></url>
</urlset>"#;
    write_sitemap(&dist, sitemap).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    orchestrator
        .validate_only()
        .await
        .expect("validate_only must pass when every sitemap URL has an html file");
}

#[tokio::test]
async fn validate_only_rejects_sitemap_with_missing_urls() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    let sitemap = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>/</loc></url>
  <url><loc>/missing</loc></url>
</urlset>"#;
    write_sitemap(&dist, sitemap).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orchestrator
        .validate_only()
        .await
        .expect_err("validate_only must fail when a sitemap URL has no html file");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("missing") || msg.contains("validation"),
        "error should mention missing URLs: {msg}"
    );
}

#[tokio::test]
async fn validate_only_rejects_malformed_sitemap_xml() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    write_sitemap(&dist, "not actually xml").await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orchestrator
        .validate_only()
        .await
        .expect_err("malformed sitemap.xml must fail validation");
    assert!(err.to_string().to_lowercase().contains("parse"));
}

#[tokio::test]
async fn validate_only_skips_unparseable_urls_in_sitemap() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    let sitemap = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>/</loc></url>
  <url><loc>relative-without-slash</loc></url>
</urlset>"#;
    write_sitemap(&dist, sitemap).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    orchestrator
        .validate_only()
        .await
        .expect("unparseable URLs must be skipped, leaving only the valid root URL");
}

#[tokio::test]
async fn validate_only_handles_empty_url_list() {
    let tmp = TempDir::new().expect("tempdir");
    let dist = tmp.path().join("dist");
    write_index(&dist).await;

    let sitemap = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>/</loc></url>
</urlset>"#;
    write_sitemap(&dist, sitemap).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    orchestrator
        .validate_only()
        .await
        .expect("single-URL sitemap pointing at / must validate");
}
