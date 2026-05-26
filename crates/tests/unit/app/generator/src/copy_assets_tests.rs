//! Drives `execute_copy_extension_assets` against a tempdir-backed
//! [`AppPaths`]. The inventory-discovered extension registry only matters
//! for whether any assets exist; here we verify the happy path when no
//! required assets are declared (returns a success JobResult).

use systemprompt_generator::execute_copy_extension_assets;
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
