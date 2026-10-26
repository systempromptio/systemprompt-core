//! Projecting a freshly swapped composition's gateway routes into the authz
//! tables.
//!
//! `services refresh` swaps the composed root the same way a boot does, so it
//! carries the same obligation: a composition whose rules were never projected
//! would grant access against the previous catalog. Each bundle is reconciled
//! under its own `bundle:<name>` source and its own ownership scope, so one
//! bundle can never revoke another's grants. The reconciled hash is recorded
//! in the cache state, which makes the next boot recognise the work as done.
//!
//! `deleted` counts gateway routes the composition stopped dispatching, so a
//! swap that retires a route reports it here rather than failing. Access rules
//! are not projected by a swap: `config apply` owns them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use serde::Serialize;
use systemprompt_governance::authz::{ReconcileReport, reconcile_composed_bundles};
use systemprompt_loader::ConfigLoader;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_manifest::Profile;
use systemprompt_manifest::services::bundle::SignedBundleManifest;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_runtime::services_reconcile::pending_composed_hash;

use crate::context::CommandContext;

const CONFIG_RELPATH: &str = "config/config.yaml";

#[derive(Debug, Serialize)]
pub struct ReconcileRow {
    pub bundle: String,
    pub registered: usize,
    pub deleted: usize,
}

pub async fn reconcile_after_swap(
    profile: &Profile,
    root: &ActiveServicesRoot,
    cache: &BundleCache,
    ctx: &CommandContext,
) -> Result<Vec<ReconcileRow>> {
    let mut state = cache.read_state();
    let Some(composed_hash) = pending_composed_hash(root, state.last_reconciled_hash.as_deref())
    else {
        return Ok(Vec::new());
    };
    let composed_hash = composed_hash.to_owned();

    let signed = cached_manifests(profile, cache, &state)?;
    let bundles: Vec<(&str, &_)> = signed
        .iter()
        .map(|(name, signed)| (name.as_str(), &signed.manifest))
        .collect();

    let services = ConfigLoader::load_from_path(&root.path.join(CONFIG_RELPATH), Some(profile))
        .context("Failed to load the composed services config")?;
    let pool = ctx
        .db_pool()
        .await
        .context("services refresh needs a database to project access rules into")?;

    let reports = reconcile_composed_bundles(&pool, &services, &bundles)
        .await
        .context("Failed to reconcile the new composition's access rules")?;

    state.last_reconciled_hash = Some(composed_hash);
    cache
        .write_state(&state)
        .context("Failed to record the reconciled composition hash")?;
    Ok(reports.iter().map(summarise).collect())
}

fn summarise((name, report): &(String, ReconcileReport)) -> ReconcileRow {
    let (registered, pruned) = report
        .gateway
        .as_ref()
        .map_or((0, 0), |gateway| (gateway.registered, gateway.pruned));
    ReconcileRow {
        bundle: name.clone(),
        registered,
        deleted: usize::try_from(pruned).unwrap_or(usize::MAX),
    }
}

fn cached_manifests(
    profile: &Profile,
    cache: &BundleCache,
    state: &systemprompt_manifest::services::bundle::ServicesBundleState,
) -> Result<Vec<(String, SignedBundleManifest)>> {
    let mut signed = Vec::with_capacity(profile.services.sources.len());
    for source in &profile.services.sources {
        let name = source.name.as_str();
        let fetched = state
            .sources
            .get(name)
            .with_context(|| format!("services bundle {name} has no cached fetch state"))?;
        let manifest = cache
            .read_manifest(name, &fetched.content_hash)
            .with_context(|| format!("services bundle {name} manifest"))?;
        signed.push((source.name.clone(), manifest));
    }
    Ok(signed)
}
