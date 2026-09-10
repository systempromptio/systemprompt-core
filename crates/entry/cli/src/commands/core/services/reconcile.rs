//! Projecting a freshly swapped composition into the authz tables.
//!
//! `services refresh` swaps the composed root the same way a boot does, so it
//! carries the same obligation: a composition whose rules were never projected
//! would grant access against the previous catalog. Each bundle is reconciled
//! under its own `bundle:<name>` source and its own ownership scope, so one
//! bundle can never revoke another's grants. The reconciled hash is recorded
//! in the cache state, which makes the next boot recognise the work as done.
//!
//! Two columns of the summary are easy to misread. `protected` counts rules an
//! operator authored in the dashboard that ingestion declined to overwrite —
//! a non-zero value is the ownership rule working, not a fault to drive to
//! zero. `deleted` counts grants a bundle stopped declaring, so a swap that
//! revokes access reports them here rather than failing.
//!
//! `inert_role_rules` names only roles. A role is checkable because it lives
//! in the `users.roles` array, so one no user holds is a dead grant. Group and
//! project values are extension-owned subject dimensions with no core table to
//! check against and are never reported, so an empty column means "no dead
//! roles found", never "every subject resolves".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use serde::Serialize;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_loader::{ActiveServicesRoot, ConfigLoader};
use systemprompt_models::Profile;
use systemprompt_models::services::bundle::SignedBundleManifest;
use systemprompt_runtime::services_reconcile::pending_composed_hash;
use systemprompt_security::authz::ingestion::IngestReport;
use systemprompt_security::authz::{ReconcileReport, reconcile_composed_bundles};

use crate::context::CommandContext;

const CONFIG_RELPATH: &str = "config/config.yaml";

#[derive(Debug, Serialize)]
pub struct ReconcileRow {
    pub bundle: String,
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
    pub protected: usize,
    pub inert_role_rules: String,
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

    let services = ConfigLoader::load_from_path(&root.path.join(CONFIG_RELPATH))
        .context("Failed to load the composed services config")?;
    let pool = ctx
        .db_pool()
        .await
        .context("services refresh needs a database to project access rules into")?;

    let reports = reconcile_composed_bundles(&pool, &services, &root.path, &bundles)
        .await
        .context("Failed to reconcile the new composition's access rules")?;

    state.last_reconciled_hash = Some(composed_hash);
    cache
        .write_state(&state)
        .context("Failed to record the reconciled composition hash")?;
    Ok(reports.iter().map(summarise).collect())
}

fn summarise((name, report): &(String, ReconcileReport)) -> ReconcileRow {
    let ingests: Vec<&IngestReport> = report
        .roles
        .iter()
        .chain(report.marketplaces.iter())
        .collect();
    let total = |pick: fn(&IngestReport) -> usize| ingests.iter().map(|r| pick(r)).sum();

    let inert: Vec<String> = ingests
        .iter()
        .flat_map(|r| r.unknown_subjects.iter())
        .map(|subject| {
            format!(
                "{} {} on {}",
                subject.rule_type, subject.value, subject.entity
            )
        })
        .collect();

    ReconcileRow {
        bundle: name.clone(),
        inserted: total(|r| r.inserted),
        updated: total(|r| r.updated),
        deleted: total(|r| r.deleted),
        protected: total(|r| r.protected),
        inert_role_rules: inert.join("; "),
    }
}

fn cached_manifests(
    profile: &Profile,
    cache: &BundleCache,
    state: &systemprompt_models::services::bundle::ServicesBundleState,
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
