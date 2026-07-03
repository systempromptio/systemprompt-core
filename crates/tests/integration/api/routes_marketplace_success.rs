//! Exercise the marketplace HTTP routes' success paths with a configured
//! marketplace on disk.
//!
//! The bootstrap fixture seeds an empty `services/marketplaces` directory; here
//! we drop a single marketplace `config.yaml` into it so `ConfigLoader::load()`
//! discovers it and the JSON / YAML render paths execute end to end. Each
//! nextest test runs in its own process, so the on-disk write is local to this
//! suite's process and never leaks into other suites.

use std::fs;
use std::sync::Arc;

use axum::Router;
use systemprompt_api::routes::marketplace;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with, fixture_db_pool,
};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

const MARKETPLACE_YAML: &str = r#"marketplace:
  id: default
  name: Default Marketplace
  description: A test marketplace for integration coverage.
  version: "1.0.0"
  enabled: true
  author:
    name: Test Author
    email: author@example.com
  license: MIT
  keywords:
    - test
"#;

async fn router_with_marketplace() -> anyhow::Result<Router> {
    let b = ensure_test_bootstrap();
    let dir = b.services_path.join("marketplaces").join("default");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("config.yaml"), MARKETPLACE_YAML)?;

    let paths = PathsConfig {
        system: b.system_path.display().to_string(),
        services: b.services_path.display().to_string(),
        bin: b.bin_path.display().to_string(),
        web_path: Some(b.system_path.join("web").display().to_string()),
        storage: Some(b.storage_path.display().to_string()),
        geoip_database: None,
    };
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))?;
    Ok(marketplace::router().with_state((*ctx).clone()))
}

#[tokio::test]
async fn marketplace_json_renders_configured_marketplace() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app.oneshot(empty_get("/marketplace.json")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200);
    assert!(body.contains("A test marketplace for integration coverage."));
    Ok(())
}

#[tokio::test]
async fn list_marketplaces_includes_configured_entry() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app.oneshot(empty_get("/marketplaces")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200);
    assert!(body.contains("default"));
    Ok(())
}

#[tokio::test]
async fn get_marketplace_by_id_renders_json() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app.oneshot(empty_get("/marketplaces/default")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200);
    assert!(body.contains("A test marketplace for integration coverage."));
    Ok(())
}

#[tokio::test]
async fn get_marketplace_yaml_serves_raw_config() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app
        .oneshot(empty_get("/marketplaces/default/manifest.yaml"))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "body: {body}");
    assert!(body.contains("Default Marketplace"), "body: {body}");
    Ok(())
}

#[tokio::test]
async fn get_unknown_marketplace_returns_not_found() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app.oneshot(empty_get("/marketplaces/nope")).await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn serve_plugin_file_unknown_plugin_returns_not_found() -> anyhow::Result<()> {
    let app = router_with_marketplace().await?;
    let resp = app
        .oneshot(empty_get("/plugins/does-not-exist/readme.md"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}
