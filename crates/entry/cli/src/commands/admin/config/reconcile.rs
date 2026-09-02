//! Re-materialise the authz catalog after a gateway or catalog edit.
//!
//! Gateway route ids are content-addressed, so changing a route's pattern or
//! provider mints a new id with no `access_control_entities` row — the next
//! request would fail closed with `UnknownEntity`. After a gateway/catalog edit
//! we make the route catalog equal to the freshly-saved services file —
//! registering the new ids and deleting the rows no route claims any more,
//! grants included — and re-apply the YAML grants against that catalog, so the
//! resolver reflects the edit without a restart or a wait for the boot-time
//! governance pass.
//!
//! Reconciliation is best-effort: the services-file write is the source of
//! truth and has already succeeded. If the database is unreachable (an offline
//! edit), we warn and return — the next app start reconciles the catalog.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_config::ProfileBootstrap;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::RouteId;
use systemprompt_models::Config;
use systemprompt_models::services::{GatewayState, ProviderRegistry};
use systemprompt_security::authz::{
    AccessControlIngestionService, AccessControlRepository, EntityKind, IngestOptions,
    RegisteredEntities, reconcile_gateway_entities_exact,
};

const ROLES_YAML_RELATIVE: &str = "access-control/roles.yaml";

pub(super) enum ReconcileOutcome {
    Reconciled,
    Deferred(String),
}

pub(super) async fn reconcile_authz(
    gateway: Option<&GatewayState>,
    providers: &ProviderRegistry,
    source_path: &str,
) -> ReconcileOutcome {
    match try_reconcile(gateway, providers, source_path).await {
        Ok(()) => ReconcileOutcome::Reconciled,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "services file saved, but the authz catalog could not be reconciled now; it will \
                 be reconciled on the next app start"
            );
            ReconcileOutcome::Deferred(err.to_string())
        },
    }
}

pub(super) fn append_reconcile_notice(message: String, outcome: &ReconcileOutcome) -> String {
    match outcome {
        ReconcileOutcome::Reconciled => message,
        ReconcileOutcome::Deferred(reason) => format!(
            "{message}\n\n⚠ authz reconcile deferred: {reason}\nThe file was saved; the authz \
             catalog will be reconciled on the next app start."
        ),
    }
}

async fn try_reconcile(
    gateway: Option<&GatewayState>,
    providers: &ProviderRegistry,
    source_path: &str,
) -> anyhow::Result<()> {
    let cfg = Config::get()?;
    let database: DbPool = Arc::new(
        Database::from_config_with_write(
            &cfg.database_type,
            &cfg.database_url,
            cfg.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await?,
    );

    let repo = AccessControlRepository::new(&database)?;
    let route_ids = gateway
        .map(|gateway| gateway.dispatchable_route_ids(providers))
        .unwrap_or_default();
    let id_refs: Vec<&str> = route_ids.iter().map(RouteId::as_str).collect();
    let source = format!("services:{source_path}");

    // Why: an empty route set is a services tree without a gateway, not an
    // instruction to empty the catalog — reconciling exactly would cascade away
    // every route grant. Leave the catalog untouched and enforce nothing in
    // that case, matching the boot job.
    let registered = if id_refs.is_empty() {
        RegisteredEntities::default()
    } else {
        reconcile_gateway_entities_exact(&repo, &id_refs, &source).await?;
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, id_refs.iter().copied())
    };

    let services_dir = ProfileBootstrap::get()?.paths.services.clone();
    let roles_yaml = Path::new(&services_dir).join(ROLES_YAML_RELATIVE);
    if roles_yaml.exists() {
        let svc = AccessControlIngestionService::new(&database)?;
        svc.ingest_config_from_yaml_path(
            &roles_yaml,
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
            &registered,
        )
        .await?;

        let services = systemprompt_loader::ConfigLoader::load()?;
        svc.ingest_marketplace_access(
            &services.marketplaces,
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await?;
    }
    Ok(())
}
