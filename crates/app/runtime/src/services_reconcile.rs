//! Authz projection of a freshly fetched services composition, run once at
//! boot.
//!
//! A composed bundle only becomes authoritative for access control after its
//! rules have been projected into the authz tables, so an instance that
//! swapped in a new composition and failed to reconcile is refused a boot
//! rather than serving the old grants against the new catalog.
//!
//! The bundles are handed to [`reconcile_composed_bundles`]
//! in profile order, which is load-bearing: the first source is the base
//! bundle and the only one whose gateway routes and `access-control` tree are
//! projected. That form prunes, so a bundle that stops declaring a grant
//! revokes it. The composition hash records only whether this composition has
//! been projected at all, which makes the step idempotent across restarts
//! that did not change the bundle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::Database;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_models::Profile;
use systemprompt_models::services::ServicesConfig;
use systemprompt_models::services::bundle::SignedBundleManifest;
use systemprompt_security::authz::reconcile_composed_bundles;

use crate::error::{RuntimeError, RuntimeResult};

#[must_use]
pub fn pending_composed_hash<'a>(
    root: &'a ActiveServicesRoot,
    last_reconciled_hash: Option<&str>,
) -> Option<&'a str> {
    match &root.provenance {
        ServicesProvenance::Fetched { composed_hash, .. }
            if last_reconciled_hash != Some(composed_hash.as_str()) =>
        {
            Some(composed_hash)
        },
        ServicesProvenance::Fetched { .. }
        | ServicesProvenance::Bundled
        | ServicesProvenance::LastGood { .. }
        | ServicesProvenance::BundledFallback { .. } => None,
    }
}

pub(crate) async fn reconcile_fetched_services(
    profile: &Profile,
    root: &ActiveServicesRoot,
    services: &ServicesConfig,
    database: &Arc<Database>,
) -> RuntimeResult<()> {
    let cache = BundleCache::new(cache_root(profile));
    let mut state = cache.read_state();
    let Some(composed_hash) = pending_composed_hash(root, state.last_reconciled_hash.as_deref())
    else {
        return Ok(());
    };
    let composed_hash = composed_hash.to_owned();

    let mut signed: Vec<(String, SignedBundleManifest)> = Vec::new();
    for source in &profile.services.sources {
        let name = source.name.as_str();
        let fetched = state.sources.get(name).ok_or_else(|| {
            RuntimeError::Internal(format!("services bundle {name} has no cached fetch state"))
        })?;
        let manifest = cache.read_manifest(name, &fetched.content_hash).map_err(|err| {
            tracing::error!(source = %name, error = %err, "Cached services bundle manifest is unreadable");
            RuntimeError::Internal(format!("services bundle {name} manifest: {err}"))
        })?;
        signed.push((source.name.clone(), manifest));
    }

    let bundles: Vec<(&str, &_)> = signed
        .iter()
        .map(|(name, signed)| (name.as_str(), &signed.manifest))
        .collect();

    let reports = reconcile_composed_bundles(database, services, &root.path, &bundles)
        .await
        .map_err(|err| {
            tracing::error!(
                composed_hash = %composed_hash,
                error = %err,
                "Refused to boot on a fetched services composition that could not be reconciled"
            );
            RuntimeError::Internal(format!("services authz reconcile: {err}"))
        })?;

    state.last_reconciled_hash = Some(composed_hash.clone());
    cache.write_state(&state).map_err(|err| {
        tracing::error!(
            composed_hash = %composed_hash,
            error = %err,
            "Reconciled a fetched services composition but could not record it"
        );
        RuntimeError::Internal(format!("services reconcile state: {err}"))
    })?;

    tracing::info!(
        composed_hash = %composed_hash,
        sources = reports.len(),
        "Projected the fetched services composition into the authz tables"
    );
    Ok(())
}
