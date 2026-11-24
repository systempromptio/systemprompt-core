//! The marketplace routes read the services config from the services view
//! the kernel seeds (the context's services watch), never from the loader per
//! request.
//!
//! A marketplace written to disk after the context is built, and primed into
//! the loader's memo, stays invisible to the routes until the `services`
//! epoch advances and the watch re-reads the tree. Each nextest test runs in
//! its own process, so the on-disk writes stay local to this suite.

use std::fs;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_test_fixtures::AppContextSeeds as _;

use async_trait::async_trait;
use axum::Router;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};
use systemprompt_traits::{CacheDomain, ConfigVersionSource, RepositoryError, WatchSignals};
use tower::ServiceExt;

use crate::common::{body_to_string, empty_get, marketplace_router};

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
    let ctx = test_app_context(&b.database_url).await;
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
        ctx.boot()
            .load_services_config()?
            .marketplaces
            .keys()
            .any(|id| id.as_str() == "late"),
        "the loader reads the new marketplace from disk"
    );

    let router = marketplace_router(&ctx);
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
    let boot = ctx.boot().clone();
    ctx.services_watch()
        .spawn(ctx.background_tasks(), signals, move || {
            let boot = boot.clone();
            async move { boot.load_services_config() }
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
    let router = marketplace_router(&ctx);
    assert_eq!(status_of(router, "/marketplaces/late").await?, 200);
    Ok(())
}
