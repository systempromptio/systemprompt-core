//! Re-materialise the authz catalog after a profile edit.
//!
//! Gateway route ids are content-addressed, so changing a route's pattern or
//! provider mints a new id with no `access_control_entities` row — the next
//! request would fail closed with `UnknownEntity`. After a gateway/catalog edit
//! we upsert the route entities from the freshly-saved profile and re-apply the
//! YAML grants, so the resolver reflects the edit without a restart or a wait
//! for the boot-time governance pass.
//!
//! Reconciliation is best-effort: the profile write is the source of truth and
//! has already succeeded. If the database is unreachable (an offline edit), we
//! warn and return — the next app start reconciles the catalog.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::RouteId;
use systemprompt_models::{Config, Profile};
use systemprompt_security::authz::{
    AccessControlIngestionService, AccessControlRepository, IngestOptions,
    reconcile_gateway_entities,
};

const ROLES_YAML_RELATIVE: &str = "access-control/roles.yaml";

// Why: Result of a post-edit authz reconciliation. `Deferred` carries the
// reason the catalog could not be re-materialised now (e.g. the database was
// unreachable during an offline edit); the profile write has already succeeded
// regardless.
pub(super) enum ReconcileOutcome {
    Reconciled,
    Deferred(String),
}

pub(super) async fn reconcile_authz(profile: &Profile, profile_path: &str) -> ReconcileOutcome {
    match try_reconcile(profile, profile_path).await {
        Ok(()) => ReconcileOutcome::Reconciled,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "profile saved, but the authz catalog could not be reconciled now; it will be \
                 reconciled on the next app start"
            );
            ReconcileOutcome::Deferred(err.to_string())
        },
    }
}

pub(super) fn append_reconcile_notice(message: String, outcome: &ReconcileOutcome) -> String {
    match outcome {
        ReconcileOutcome::Reconciled => message,
        ReconcileOutcome::Deferred(reason) => format!(
            "{message}\n\n⚠ authz reconcile deferred: {reason}\nThe profile was saved; the authz \
             catalog will be reconciled on the next app start."
        ),
    }
}

async fn try_reconcile(profile: &Profile, profile_path: &str) -> anyhow::Result<()> {
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
    let route_ids = profile
        .gateway
        .as_ref()
        .map(|gateway| gateway.dispatchable_route_ids(&profile.providers))
        .unwrap_or_default();
    let id_refs: Vec<&str> = route_ids.iter().map(RouteId::as_str).collect();
    let source = format!("profile:{profile_path}");
    reconcile_gateway_entities(&repo, &id_refs, &source).await?;

    let roles_yaml = Path::new(&profile.paths.services).join(ROLES_YAML_RELATIVE);
    if roles_yaml.exists() {
        let svc = AccessControlIngestionService::new(&database)?;
        svc.ingest_config_from_yaml_path(
            &roles_yaml,
            IngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
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
