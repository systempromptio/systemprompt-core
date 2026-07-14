//! Filesystem error arms in the build steps and asset reorganisation:
//! `organize_css` failing to create `dist/css` or to copy a CSS file, and
//! `organize_dist_assets` failing to copy a matching entry.

use std::fs;

use systemprompt_generator::{BuildError, BuildMode, BuildOrchestrator, organize_dist_assets};
use tempfile::TempDir;

#[tokio::test]
async fn build_fails_when_css_dir_is_a_file() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), "<html></html>").unwrap();
    fs::write(dist.join("css"), "not a directory").unwrap();

    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orch.build().await.expect_err("css dir creation must fail");
    assert!(
        matches!(err, BuildError::CssOrganizationFailed(ref m) if m.contains("create css directory")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn build_fails_when_css_copy_destination_is_a_directory() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(dist.join("css/content.css")).unwrap();
    fs::write(dist.join("index.html"), "<html></html>").unwrap();
    fs::write(dist.join("content.css"), "body {}").unwrap();

    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Production);
    let err = orch.build().await.expect_err("css copy must fail");
    assert!(
        matches!(err, BuildError::CssOrganizationFailed(ref m) if m.contains("content.css")),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn validate_only_passes_on_minimal_dist() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), "<html></html>").unwrap();

    let orch = BuildOrchestrator::new(tmp.path().to_path_buf(), BuildMode::Development);
    orch.validate_only().await.expect("validation passes");
    assert_eq!(orch.mode(), BuildMode::Development);
}

#[tokio::test]
async fn organize_dist_assets_errors_when_entry_is_uncopyable() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().to_path_buf();
    fs::create_dir_all(dist.join("fake.css")).unwrap();

    let err = organize_dist_assets(&dist)
        .await
        .expect_err("copying a directory named *.css must fail");
    let msg = err.to_string();
    assert!(msg.contains("fake.css"), "unexpected error: {msg}");
}
