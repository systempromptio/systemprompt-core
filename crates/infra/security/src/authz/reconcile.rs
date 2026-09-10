//! Projection of a services tree into the authz tables.
//!
//! Three passes run against the tree, in order: the gateway-route catalog is
//! made equal to the routes the tree dispatches, `access-control/roles.yaml` is
//! ingested against that catalog, and every marketplace `access` block is
//! projected. Route ids are content-addressed, so the catalog pass has to come
//! first — a rule naming a route the tree no longer defines is rejected rather
//! than materialised.
//!
//! Every caller passes the `source` that owns the rows it writes and the
//! [`IngestScope`] it is entitled to prune inside. A tree fetched as
//! `bundle:<name>` therefore cannot revoke another bundle's grants, and no pass
//! touches what an operator authored in the dashboard.
//!
//! [`reconcile_services_authz`] is the whole-tree form, for a tree with one
//! owner: the baked services directory behind `admin config reconcile`.
//! [`reconcile_composed_bundles`] is the form for a composed root, where each
//! bundle owns a slice of the same tree — boot and `services refresh` both go
//! through it so the two cannot drift. Only that form prunes: a bundle that
//! drops a grant must revoke it, and `source` plus the bundle's ownership scope
//! bound the delete to rows that bundle itself wrote. The whole-tree form
//! leaves orphans alone, because an operator editing one file by hand has not
//! declared anything about the rest of the tree.
//!
//! # What a bundle's ownership maps onto
//!
//! Four of `BundleOwnership`'s lists name authz entities and become scope
//! kinds: `marketplaces`, `plugins`, `skills` and `hooks`. `rules`, `artifacts`
//! and `dirs` have no [`EntityKind`] because they are not things access control
//! decides about — `rules` are prompt content, `artifacts` are rendered output,
//! and `dirs` is a packing-time claim on the tree layout. Nothing is derived
//! from them here, and a new ownership list only belongs in the scope once the
//! resolver has a kind to decide about it.
//!
//! The gateway-route catalog is reconciled once, against the base bundle,
//! never once per source. The catalog pass is an exact set reconcile: running
//! it per source would converge, but each run would relabel the entity rows
//! with whichever source happened to go last, so provenance would be a race.
//! Only the base bundle may carry `gateway/` and `access-control/`, so only it
//! gets those two passes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::Path;

use systemprompt_database::DbPool;
use systemprompt_identifiers::RouteId;
use systemprompt_models::services::{BundleOwnership, ServicesBundleManifest, ServicesConfig};

use super::error::AuthzResult;
use super::gateway_entities::{GatewayReconcileReport, reconcile_gateway_entities_exact};
use super::ingestion::{
    AccessControlIngestionService, IngestOptions, IngestReport, IngestScope, RegisteredEntities,
};
use super::repository::AccessControlRepository;
use super::types::EntityKind;

const ROLES_YAML_RELATIVE: &str = "access-control/roles.yaml";

#[derive(Debug, Clone, Default)]
pub struct ReconcileReport {
    pub gateway: Option<GatewayReconcileReport>,
    pub roles: Option<IngestReport>,
    pub marketplaces: Option<IngestReport>,
}

struct Pass<'a> {
    source: &'a str,
    scope: IngestScope,
    platform_dirs: bool,
    delete_orphans: bool,
}

pub async fn reconcile_services_authz(
    db: &DbPool,
    services: &ServicesConfig,
    services_root: &Path,
    source: &str,
    scope: Option<IngestScope>,
) -> AuthzResult<ReconcileReport> {
    let pass = Pass {
        source,
        scope: scope.unwrap_or_default(),
        platform_dirs: true,
        delete_orphans: false,
    };
    run_pass(db, services, services_root, &pass).await
}

pub async fn reconcile_composed_bundles(
    db: &DbPool,
    services: &ServicesConfig,
    composed_root: &Path,
    bundles: &[(&str, &ServicesBundleManifest)],
) -> AuthzResult<Vec<(String, ReconcileReport)>> {
    let mut out = Vec::with_capacity(bundles.len());
    for (index, (name, manifest)) in bundles.iter().enumerate() {
        let owns = &manifest.owns;
        let view = bundle_view(services, owns, index == 0);
        let source = format!("bundle:{name}");
        let pass = Pass {
            source: &source,
            scope: bundle_scope(owns, &view, index == 0),
            platform_dirs: index == 0,
            delete_orphans: true,
        };
        let report = run_pass(db, &view, composed_root, &pass).await?;
        out.push(((*name).to_owned(), report));
    }
    Ok(out)
}

async fn run_pass(
    db: &DbPool,
    services: &ServicesConfig,
    services_root: &Path,
    pass: &Pass<'_>,
) -> AuthzResult<ReconcileReport> {
    let repo = AccessControlRepository::new(db)?;
    let svc = AccessControlIngestionService::new(db)?;

    let route_ids = services
        .gateway
        .as_ref()
        .map(|gateway| gateway.dispatchable_route_ids(&services.providers))
        .unwrap_or_default();
    let id_refs: Vec<&str> = route_ids.iter().map(RouteId::as_str).collect();

    let mut report = ReconcileReport::default();
    let registered = if id_refs.is_empty() {
        RegisteredEntities::default()
    } else {
        report.gateway =
            Some(reconcile_gateway_entities_exact(&repo, &id_refs, pass.source).await?);
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, id_refs.iter().copied())
    };

    let options = IngestOptions {
        override_existing: true,
        delete_orphans: pass.delete_orphans,
        source: pass.source.to_owned(),
        scope: pass.scope.clone(),
    };

    let roles_yaml = services_root.join(ROLES_YAML_RELATIVE);
    if pass.platform_dirs && roles_yaml.exists() {
        report.roles = Some(
            svc.ingest_config_from_yaml_path(&roles_yaml, options.clone(), &registered)
                .await?,
        );
    }

    report.marketplaces = Some(
        svc.ingest_marketplace_access(&services.marketplaces, options)
            .await?,
    );

    Ok(report)
}

fn bundle_view(services: &ServicesConfig, owns: &BundleOwnership, is_base: bool) -> ServicesConfig {
    let owned: HashSet<&str> = owns.marketplaces.iter().map(String::as_str).collect();
    let mut view = services.clone();
    view.marketplaces
        .retain(|id, _| owned.contains(id.as_str()));
    if !is_base {
        view.gateway = None;
    }
    view
}

fn bundle_scope(owns: &BundleOwnership, view: &ServicesConfig, is_base: bool) -> IngestScope {
    let mut scope = IngestScope::new()
        .with_kind(EntityKind::Marketplace, owns.marketplaces.clone())
        .with_kind(EntityKind::Plugin, owns.plugins.clone())
        .with_kind(EntityKind::Skill, owns.skills.clone())
        .with_kind(EntityKind::Hook, owns.hooks.clone());
    if is_base {
        let route_ids = view
            .gateway
            .as_ref()
            .map(|gateway| gateway.dispatchable_route_ids(&view.providers))
            .unwrap_or_default();
        scope = scope.with_kind(
            EntityKind::GatewayRoute,
            route_ids.iter().map(|id| id.as_str().to_owned()),
        );
    }
    scope
}
