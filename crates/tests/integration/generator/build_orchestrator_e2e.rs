//! End-to-end build pipeline tests: drive `BuildOrchestrator::build()` and
//! `validate_only()` against on-disk `dist/` scaffolds and assert that the
//! orchestrator wires `organize_css` and `validate_build` together
//! correctly.

use std::path::Path;
use systemprompt_generator::{BuildMode, BuildOrchestrator};
use tempfile::TempDir;
use tokio::fs;

async fn make_dist_scaffold(web_dir: &Path) {
    let dist = web_dir.join("dist");
    fs::create_dir_all(&dist).await.expect("mkdir dist");
    fs::write(dist.join("index.html"), "<html></html>")
        .await
        .expect("write index.html");
}

#[tokio::test]
async fn build_succeeds_on_minimal_scaffold() {
    let tmp = TempDir::new().expect("tempdir");
    make_dist_scaffold(tmp.path()).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    orchestrator
        .build()
        .await
        .expect("minimal build should succeed");

    assert!(tmp.path().join("dist/css").exists());
}

#[tokio::test]
async fn build_fails_when_dist_missing() {
    let tmp = TempDir::new().expect("tempdir");

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orchestrator
        .build()
        .await
        .expect_err("build must fail with no dist/");

    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("dist") || msg.contains("validation"),
        "error must mention missing dist or validation: {msg}"
    );
}

#[tokio::test]
async fn build_fails_when_index_html_missing() {
    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("dist"))
        .await
        .expect("mkdir dist");

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orchestrator
        .build()
        .await
        .expect_err("build must fail with no index.html");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("index.html") || msg.contains("validation"),
        "error must mention missing index.html: {msg}"
    );
}

#[tokio::test]
async fn build_organizes_known_css_files() {
    let tmp = TempDir::new().expect("tempdir");
    make_dist_scaffold(tmp.path()).await;

    let dist = tmp.path().join("dist");
    fs::write(dist.join("content.css"), "/* content */")
        .await
        .expect("write content.css");
    fs::write(dist.join("syntax-highlight.css"), "/* syntax */")
        .await
        .expect("write syntax-highlight.css");

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    orchestrator.build().await.expect("build must succeed");

    assert!(
        dist.join("css/content.css").exists(),
        "content.css must be copied into dist/css/"
    );
    assert!(
        dist.join("css/syntax-highlight.css").exists(),
        "syntax-highlight.css must be copied into dist/css/"
    );
    let copied = fs::read_to_string(dist.join("css/content.css"))
        .await
        .expect("read copied content.css");
    assert_eq!(copied, "/* content */");
}

#[tokio::test]
async fn build_skips_missing_known_css_files() {
    let tmp = TempDir::new().expect("tempdir");
    make_dist_scaffold(tmp.path()).await;

    let orchestrator = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Docker);
    orchestrator
        .build()
        .await
        .expect("build must succeed even when known css files are absent");

    let dist = tmp.path().join("dist");
    assert!(dist.join("css").exists(), "css dir must be created");
    assert!(
        !dist.join("css/content.css").exists(),
        "no css file to copy means none should appear"
    );
}

#[tokio::test]
async fn build_mode_is_preserved() {
    let tmp = TempDir::new().expect("tempdir");
    make_dist_scaffold(tmp.path()).await;

    let dev = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    assert_eq!(dev.mode(), BuildMode::Development);
    let prod = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    assert_eq!(prod.mode(), BuildMode::Production);
    let docker = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Docker);
    assert_eq!(docker.mode(), BuildMode::Docker);
}
