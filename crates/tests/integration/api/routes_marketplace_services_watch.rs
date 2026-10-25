//! The marketplace routes read the services config from the `AppContext`
//! services watch, never from the loader per request.
//!
//! A marketplace written to disk after the context is built, and primed into
//! the loader's memo, stays invisible to the routes until the `services`
//! epoch advances and the watch re-reads the tree. Each nextest test runs in
//! its own process, so the on-disk writes stay local to this suite.

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::Router;
use systemprompt_api::routes::marketplace;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context_with, test_db_pool};
use systemprompt_traits::{CacheDomain, ConfigVersionSource, RepositoryError, WatchSignals};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

const LATE_YAML: &str = r#"marketplace:
  id: late
  name: Late Marketplace
  description: Written after the context was built.
  version: "1.0.0"
  enabled: true
  author:
    name: Test Author
    email: author@example.com
  license: MIT
"#;

struct FixedVersion;

#[async_trait]
impl ConfigVersionSource for FixedVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(0)
    }
}

async fn context() -> anyhow::Result<AppContext> {
    let b = ensure_test_bootstrap();
    let paths = PathsConfig {
        system: b.system_path.display().to_string(),
        services: b.services_path.display().to_string(),
        bin: b.bin_path.display().to_string(),
        web_path: Some(b.system_path.join("web").display().to_string()),
        storage: Some(b.storage_path.display().to_string()),
        geoip_database: None,
        shipped_files: None,
        data: None,
    };
    let pool = test_db_pool().await;
    let ctx = fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))?;
    Ok((*ctx).clone())
}

fn write_late_marketplace() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let dir = b.services_path.join("marketplaces").join("late");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("config.yaml"), LATE_YAML)?;
    Ok(())
}

async fn status_of(router: Router, uri: &str) -> anyhow::Result<u16> {
    let (status, _) = body_to_string(router.oneshot(empty_get(uri)).await?).await?;
    Ok(status.as_u16())
}

#[tokio::test]
async fn a_tree_change_is_not_served_until_the_watch_reloads() -> anyhow::Result<()> {
    let ctx = context().await?;
    write_late_marketplace()?;
    assert!(
        ConfigLoader::load()?
            .marketplaces
            .keys()
            .any(|id| id.as_str() == "late"),
        "the loader reads the new marketplace from disk"
    );

    let router = marketplace::router().with_state(ctx.clone());
    assert_eq!(
        status_of(router, "/marketplaces/late").await?,
        404,
        "the route must answer from the watch, not a per-request load"
    );
    Ok(())
}

#[tokio::test]
async fn advancing_the_services_epoch_serves_the_new_tree() -> anyhow::Result<()> {
    let ctx = context().await?;
    write_late_marketplace()?;

    let signals = WatchSignals {
        domain: CacheDomain::Services,
        epochs: ctx.cache_epochs().clone(),
        version: Arc::new(FixedVersion),
        poll: Duration::from_secs(3600),
    };
    ctx.services_watch()
        .spawn(ctx.background_tasks(), signals, || async {
            ConfigLoader::load()
        });
    for _ in 0..200 {
        if ctx.services_watch().reloads() > 0 {
            break;
        }
        ctx.cache_epochs().advance(CacheDomain::Services);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        ctx.services_watch().reloads() > 0,
        "the epoch reloads the watch"
    );
    let router = marketplace::router().with_state(ctx.clone());
    assert_eq!(status_of(router, "/marketplaces/late").await?, 200);
    Ok(())
}
