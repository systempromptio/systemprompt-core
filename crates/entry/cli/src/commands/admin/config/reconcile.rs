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

use std::path::Path;
use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::RouteId;
use systemprompt_models::{Config, Profile};
use systemprompt_security::authz::{AccessControlRepository, reconcile_gateway_entities};
use systemprompt_sync::AccessControlLocalSync;

const ROLES_YAML_RELATIVE: &str = "access-control/roles.yaml";

pub(super) async fn reconcile_authz(profile: &Profile, profile_path: &str) {
    if let Err(err) = try_reconcile(profile, profile_path).await {
        tracing::warn!(
            error = %err,
            "profile saved, but the authz catalog could not be reconciled now; it will be \
             reconciled on the next app start"
        );
    }
}

async fn try_reconcile(profile: &Profile, profile_path: &str) -> anyhow::Result<()> {
    let cfg = Config::get()?;
    let database: DbPool = Arc::new(
        Database::from_config_with_write(
            &cfg.database_type,
            &cfg.database_url,
            cfg.database_write_url.as_deref(),
        )
        .await?,
    );

    let repo = AccessControlRepository::new(&database)?;
    let route_ids = profile
        .gateway
        .as_ref()
        .map(systemprompt_models::profile::GatewayState::resolved_route_ids)
        .unwrap_or_default();
    let id_refs: Vec<&str> = route_ids.iter().map(RouteId::as_str).collect();
    let source = format!("profile:{profile_path}");
    reconcile_gateway_entities(&repo, &id_refs, &source).await?;

    let roles_yaml = Path::new(&profile.paths.services).join(ROLES_YAML_RELATIVE);
    if roles_yaml.exists() {
        AccessControlLocalSync::new(database, roles_yaml)
            .sync_to_db(true, false)
            .await?;
    }
    Ok(())
}
