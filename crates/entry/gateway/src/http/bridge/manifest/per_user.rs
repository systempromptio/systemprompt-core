//! Per-consumer inputs to the signed manifest: the user row, revoked
//! credentials, host preferences, and the catalogue grants the manifest
//! records for the organisation skills it carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, ManagedResourceId, UserId};
use systemprompt_marketplace::MarketplaceCandidate;
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_models::bridge::manifest::UserInfo;

use super::super::BridgeRouteState;
use super::super::data;
use super::super::error::BridgeError;

// Why: the recorded grant is the consumer's reach — a manifest whose grants
// could not be written must not serve the skills it would grant.
pub(super) async fn record_catalog_grants(
    state: &BridgeRouteState,
    user_id: &UserId,
    candidate: &MarketplaceCandidate,
) -> Result<(), BridgeError> {
    let owner = state.settings.system_admin();
    if user_id == owner {
        return Ok(());
    }
    let started = std::time::Instant::now();
    let resources: Vec<ManagedResourceId> = candidate
        .skills
        .iter()
        .filter_map(|skill| skill.publication.as_ref())
        .map(|publication| publication.resource_id.clone())
        .collect();
    state
        .marketplace
        .managed
        .retain_consumer_catalog_grants(owner, &resources, user_id)
        .await
        .map_err(|source| BridgeError::CatalogGrants {
            count: resources.len(),
            source,
        })?;
    tracing::debug!(
        grants = resources.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "bridge: manifest catalogue grants recorded"
    );
    Ok(())
}

pub(super) struct PerUserContext {
    pub user: Option<UserInfo>,
    pub revocations: Vec<ApiKeyId>,
    pub enabled_hosts: Vec<HostKind>,
    pub host_model_protocols: std::collections::BTreeMap<HostKind, Vec<String>>,
}

// Why: `revocations` and `enabled_hosts` are the policy half of the signed
// manifest — a failed read must not be served as "nothing revoked" / "every
// host enabled". Only the display and preference inputs may degrade.
pub(super) async fn load_per_user_context(
    state: &BridgeRouteState,
    user_id: &UserId,
    instance_hosts: InstanceHosts,
) -> Result<PerUserContext, BridgeError> {
    let user = match data::load_user(state, user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: user load failed; continuing without user");
            None
        },
    };

    let revocations = data::load_revocations(state, user_id)
        .await
        .map_err(|e| BridgeError::internal("manifest: revocations unavailable", e))?;

    let enabled_hosts = match data::load_enabled_hosts(state, user_id).await {
        Ok(prefs) => instance_hosts
            .enabled
            .iter()
            .copied()
            .filter(|host| prefs.admits(*host, &instance_hosts))
            .collect(),
        Err(e) => {
            return Err(BridgeError::internal(
                "manifest: enabled hosts unavailable",
                e,
            ));
        },
    };

    let host_model_protocols = match data::load_host_model_protocols(state, user_id).await {
        Ok(rows) => rows.into_iter().collect(),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "manifest: host model-protocol prefs load failed; continuing with defaults"
            );
            std::collections::BTreeMap::new()
        },
    };

    Ok(PerUserContext {
        user,
        revocations,
        enabled_hosts,
        host_model_protocols,
    })
}
