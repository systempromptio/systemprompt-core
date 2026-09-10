//! Re-materialise the authz catalog after a gateway or catalog edit.
//!
//! Gateway route ids are content-addressed, so changing a route's pattern or
//! provider mints a new id with no `access_control_entities` row — the next
//! request would fail closed with `UnknownEntity`. After a gateway/catalog edit
//! we hand the freshly-saved services tree to
//! [`reconcile_services_authz`], so the resolver reflects the edit without a
//! restart or a wait for the boot-time governance pass.
//!
//! The edit is projected under the `yaml` source: this is the baked services
//! tree, not a fetched bundle, and it prunes unscoped because it is the only
//! writer of those rows.
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
use systemprompt_models::Config;
use systemprompt_models::services::{GatewayState, ProviderRegistry};
use systemprompt_security::authz::{YAML_SOURCE, reconcile_services_authz};

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

    let mut services = systemprompt_loader::ConfigLoader::load()?;
    services.providers = providers.clone();
    if let Some(gateway) = gateway {
        services.gateway = Some(gateway.clone());
    }

    let services_dir = ProfileBootstrap::get()?.paths.services.clone();
    let source = format!("services:{source_path}");
    reconcile_services_authz(
        &database,
        &services,
        Path::new(&services_dir),
        YAML_SOURCE,
        None,
    )
    .await?;
    tracing::debug!(source = %source, "authz reconciled after a services edit");
    Ok(())
}
