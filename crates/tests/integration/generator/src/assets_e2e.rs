//! Drives `organize_dist_assets` through scenarios not already covered by
//! the unit-test crate: case-sensitive extension matching, files with no
//! extension, and mixed content alongside CSS/JS.

use std::path::Path;
use systemprompt_generator::organize_dist_assets;
use tempfile::TempDir;
use tokio::fs;

async fn touch(dir: &Path, name: &str) {
    fs::write(dir.join(name), "x")
        .await
        .unwrap_or_else(|e| panic!("write {name}: {e}"));
}

#[tokio::test]
async fn organize_excludes_uppercase_extensions() {
    let tmp = TempDir::new().expect("tempdir");
    touch(tmp.path(), "lower.css").await;
    touch(tmp.path(), "UPPER.CSS").await;
    touch(tmp.path(), "lower.js").await;
    touch(tmp.path(), "UPPER.JS").await;

    let (css_count, js_count) = organize_dist_assets(tmp.path())
        .await
        .expect("organize must succeed");
    assert_eq!(css_count, 1, "extension match is case-sensitive on css");
    assert_eq!(js_count, 1, "extension match is case-sensitive on js");
    assert!(tmp.path().join("css/lower.css").exists());
    assert!(tmp.path().join("js/lower.js").exists());
}

#[tokio::test]
async fn organize_ignores_files_with_no_extension() {
    let tmp = TempDir::new().expect("tempdir");
    touch(tmp.path(), "README").await;
    touch(tmp.path(), "Makefile").await;
    touch(tmp.path(), "site.css").await;

    let (css_count, js_count) = organize_dist_assets(tmp.path())
        .await
        .expect("organize must succeed");
    assert_eq!(css_count, 1);
    assert_eq!(js_count, 0);
}

#[tokio::test]
async fn organize_returns_zero_on_only_non_asset_files() {
    let tmp = TempDir::new().expect("tempdir");
    touch(tmp.path(), "index.html").await;
    touch(tmp.path(), "data.json").await;
    touch(tmp.path(), "image.png").await;

    let (css_count, js_count) = organize_dist_assets(tmp.path())
        .await
        .expect("organize must succeed");
    assert_eq!(css_count, 0);
    assert_eq!(js_count, 0);
    assert!(tmp.path().join("css").exists());
    assert!(tmp.path().join("js").exists());
}
