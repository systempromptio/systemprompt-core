//! Seeds the `governance` config plane the way `config apply` does and makes
//! a fixture context's engine follow it.
//!
//! The engine reads `governance_config`, never a `services/governance` file,
//! so a test that needs a declared chain writes the plane's rows and then
//! drives the context's governance watch through one epoch reload. A test
//! whose plane is deliberately empty records the plane as applied instead.

use std::time::Duration;

use systemprompt_database::{DbPool, PgConfigVersion, PlaneTransaction};
use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, PlaneContext, PlaneTx, ResolvedEntities, ResolvedEntity,
};
use systemprompt_governance::policy::{GovernancePlane, GOVERNANCE_FILE};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{CacheDomain, Dependencies, WatchSignals};

pub async fn apply_governance_plane(pool: &DbPool, yaml: &str) -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join(GOVERNANCE_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, yaml)?;
    let declared = GovernancePlane
        .load_declared(root.path())?
        .ok_or_else(|| anyhow::anyhow!("governance declaration is empty"))?;
    let resolved: ResolvedEntities = declared
        .entries
        .into_iter()
        .map(|(key, value)| {
            let source = ChangeSource::Code(GOVERNANCE_FILE.to_owned());
            (
                key,
                ResolvedEntity {
                    value: Some(value),
                    source,
                },
            )
        })
        .collect();
    let deps = Dependencies::new();
    let ctx = PlaneContext::new(root.path(), &deps);
    let mut tx: PlaneTransaction = pool.write_pool().begin().await?;
    GovernancePlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved)
        .await?;
    tx.commit().await?;
    Ok(())
}

// Records `plane` as applied the way `config apply` does, for a test whose
// declaration is deliberately empty.
pub async fn record_plane_applied(pool: &DbPool, plane: &str) -> anyhow::Result<()> {
    let mut conn = pool.write_pool().acquire().await?;
    systemprompt_database::record_config_plane_applied(&mut conn, plane).await?;
    Ok(())
}

pub async fn apply_governance(ctx: &AppContext, yaml: &str) -> anyhow::Result<()> {
    apply_governance_plane(ctx.db_pool(), yaml).await?;
    let signals = WatchSignals {
        domain: CacheDomain::Governance,
        epochs: ctx.cache_epochs().clone(),
        version: std::sync::Arc::new(PgConfigVersion::new(ctx.db_pool())),
        poll: Duration::from_secs(3600),
    };
    let reload_ctx = ctx.clone();
    ctx.governance_watch()
        .spawn(ctx.background_tasks(), signals, move || {
            let ctx = reload_ctx.clone();
            async move { ctx.load_governance().await }
        });
    let start = ctx.governance_watch().reloads();
    for _ in 0..500 {
        if ctx.governance_watch().reloads() > start {
            return Ok(());
        }
        ctx.cache_epochs().advance(CacheDomain::Governance);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    anyhow::bail!("governance watch never reloaded the applied chain")
}
