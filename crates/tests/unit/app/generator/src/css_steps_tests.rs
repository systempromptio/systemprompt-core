use std::fs;
use systemprompt_generator::{BuildMode, BuildOrchestrator};
use tempfile::TempDir;

fn make_orchestrator(tmp: &TempDir) -> BuildOrchestrator {
    BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production)
}

#[tokio::test]
async fn build_creates_css_directory_inside_dist() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
    assert!(dist.join("css").is_dir());
}

#[tokio::test]
async fn build_copies_content_css_when_present() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("content.css"), b"body { margin: 0; }").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
    assert!(dist.join("css").join("content.css").exists());
    let content = fs::read_to_string(dist.join("css").join("content.css")).unwrap();
    assert_eq!(content, "body { margin: 0; }");
}

#[tokio::test]
async fn build_copies_syntax_highlight_css_when_present() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("syntax-highlight.css"), b"pre { color: #000; }").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
    assert!(dist.join("css").join("syntax-highlight.css").exists());
    let content = fs::read_to_string(dist.join("css").join("syntax-highlight.css")).unwrap();
    assert_eq!(content, "pre { color: #000; }");
}

#[tokio::test]
async fn build_skips_missing_css_files_gracefully() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
    assert!(!dist.join("css").join("content.css").exists());
    assert!(!dist.join("css").join("syntax-highlight.css").exists());
}

#[tokio::test]
async fn build_copies_both_css_files_together() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("content.css"), b"a { color: blue; }").unwrap();
    fs::write(dist.join("syntax-highlight.css"), b"code { font: mono; }").unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
    assert!(dist.join("css").join("content.css").exists());
    assert!(dist.join("css").join("syntax-highlight.css").exists());
}

#[tokio::test]
async fn build_fails_when_dist_directory_missing() {
    let tmp = TempDir::new().unwrap();
    let orch = make_orchestrator(&tmp);
    let result = orch.build().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn build_fails_when_index_html_missing_from_dist() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("dist")).unwrap();
    let orch = make_orchestrator(&tmp);
    let result = orch.build().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn build_dev_mode_behaves_same_as_production() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("content.css"), b".x {}").unwrap();
    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    orch.build().await.unwrap();
    assert!(dist.join("css").join("content.css").exists());
}

#[tokio::test]
async fn build_docker_mode_behaves_same_as_production() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Docker);
    orch.build().await.unwrap();
    assert!(dist.join("css").is_dir());
}

#[tokio::test]
async fn build_with_sitemap_validates_all_referenced_urls() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    let about_dir = dist.join("about");
    fs::create_dir_all(&about_dir).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(about_dir.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(
        dist.join("sitemap.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/</loc></url>
  <url><loc>https://example.com/about</loc></url>
</urlset>"#,
    )
    .unwrap();
    let orch = make_orchestrator(&tmp);
    orch.build().await.unwrap();
}

#[tokio::test]
async fn build_with_invalid_sitemap_xml_returns_validation_error() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), b"<!doctype html>").unwrap();
    fs::write(dist.join("sitemap.xml"), b"not valid xml at all").unwrap();
    let orch = make_orchestrator(&tmp);
    let err = orch.build().await.unwrap_err();
    assert!(err.to_string().contains("Validation failed"));
}
