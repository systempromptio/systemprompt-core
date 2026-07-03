//! Drives `execute_copy_extension_assets` against a tempdir-backed
//! [`AppPaths`]. The inventory-discovered extension registry only matters
//! for whether any assets exist; here we verify the happy path when no
//! required assets are declared (returns a success JobResult).

use systemprompt_extension::AssetDefinition;
use systemprompt_generator::{copy_asset, execute_copy_extension_assets};
use systemprompt_models::AppPaths;
use systemprompt_models::profile::PathsConfig;
use tempfile::TempDir;

fn make_app_paths(tmp: &TempDir) -> AppPaths {
    let p = tmp.path().to_string_lossy().to_string();
    AppPaths::from_profile(&PathsConfig {
        system: p.clone(),
        services: p.clone(),
        bin: p.clone(),
        web_path: Some(p.clone()),
        storage: Some(p),
        geoip_database: None,
    })
    .expect("paths")
}

#[tokio::test]
async fn execute_copy_extension_assets_returns_success() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("dist")).unwrap();
    let paths = make_app_paths(&tmp);
    let res = execute_copy_extension_assets(&paths).await;
    assert!(res.is_ok(), "must succeed: {res:?}");
    let job = res.unwrap();
    assert!(job.success);
}

#[tokio::test]
async fn copy_asset_creates_parent_dir_and_copies_file() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("source.css");
    std::fs::write(&src, b"body { color: red; }").unwrap();
    let dist = tmp.path().join("dist");
    std::fs::create_dir_all(&dist).unwrap();

    let asset = AssetDefinition::css(src.clone(), "nested/deep/style.css");
    let res = copy_asset(&dist, "ext-under-test", &asset).await;
    assert!(res.is_ok(), "copy must succeed: {res:?}");

    let dest = dist.join("nested/deep/style.css");
    assert!(dest.exists(), "destination file must exist");
    assert_eq!(
        std::fs::read_to_string(&dest).unwrap(),
        "body { color: red; }"
    );
}

#[tokio::test]
async fn copy_asset_missing_source_errors() {
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    std::fs::create_dir_all(&dist).unwrap();

    let missing = tmp.path().join("does-not-exist.css");
    let asset = AssetDefinition::css(missing, "out/style.css");
    let res = copy_asset(&dist, "ext-under-test", &asset).await;
    assert!(res.is_err(), "missing source must error");
}
