//! Projection of a services tree's gateway-route catalog into the authz
//! tables.
//!
//! The gateway-route catalog is made equal to the routes the tree dispatches.
//! `access-control/rules.yaml` and marketplace `access` blocks are not
//! projected here: they are the `access_control` config plane, which only
//! `config apply` (or `/admin/sync`) writes, so nothing in a boot or a
//! `services refresh` grants or revokes access. The catalog pass registers
//! entity rows only; it never touches `default_included` on an existing row
//! and never prunes a glob entity the access plane declared.
//!
//! Every caller passes the `source` that owns the rows it writes.
//! [`reconcile_services_authz`] is the whole-tree form, for a tree with one
//! owner: the baked services directory behind `admin config reconcile`.
//! [`reconcile_composed_bundles`] is the form for a composed root; boot and
//! `services refresh` both go through it so the two cannot drift. The catalog
//! is reconciled once, against the base bundle, never once per source: the
//! pass is an exact set reconcile, so running it per source would relabel the
//! entity rows with whichever source happened to go last. Only the base
//! bundle may carry `gateway/`, so only it gets the pass.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::DbPool;
use systemprompt_identifiers::RouteId;
use systemprompt_manifest::services::catalog::CatalogContext;
use systemprompt_manifest::services::{ServicesBundleManifest, ServicesConfig};

use super::error::AuthzResult;
use super::gateway_entities::{GatewayReconcileReport, reconcile_gateway_entities_exact};
use super::repository::AccessControlRepository;

#[derive(Debug, Clone, Copy, Default)]
pub struct ReconcileReport {
    pub gateway: Option<GatewayReconcileReport>,
}

pub async fn reconcile_services_authz(
    db: &DbPool,
    services: &ServicesConfig,
    source: &str,
) -> AuthzResult<ReconcileReport> {
    reconcile_catalog(db, services, source).await
}

pub async fn reconcile_composed_bundles(
    db: &DbPool,
    services: &ServicesConfig,
    bundles: &[(&str, &ServicesBundleManifest)],
) -> AuthzResult<Vec<(String, ReconcileReport)>> {
    let mut out = Vec::with_capacity(bundles.len());
    for (index, (name, _manifest)) in bundles.iter().enumerate() {
        let report = if index == 0 {
            reconcile_catalog(db, services, &format!("bundle:{name}")).await?
        } else {
            ReconcileReport::default()
        };
        out.push(((*name).to_owned(), report));
    }
    Ok(out)
}

async fn reconcile_catalog(
    db: &DbPool,
    services: &ServicesConfig,
    source: &str,
) -> AuthzResult<ReconcileReport> {
    let route_ids = dispatchable_route_ids(services);
    let id_refs: Vec<&str> = route_ids.iter().map(RouteId::as_str).collect();

    let mut report = ReconcileReport::default();
    if !id_refs.is_empty() {
        let repo = AccessControlRepository::new(db);
        report.gateway = Some(reconcile_gateway_entities_exact(&repo, &id_refs, source).await?);
    }
    Ok(report)
}

fn dispatchable_route_ids(services: &ServicesConfig) -> Vec<RouteId> {
    if services.gateway.is_none() {
        return Vec::new();
    }
    match services.resolve_catalog(&CatalogContext::default()) {
        Ok(catalog) => catalog.route_ids(),
        Err(error) => {
            tracing::warn!(%error, "catalog does not resolve; no gateway route is reconciled");
            Vec::new()
        },
    }
}
