//! Gateway-route entity catalog reconciliation.
//!
//! Gateway route ids are content-addressed (`synthesize_route_id` in
//! `systemprompt_models`), so they are stable across installs but change the
//! moment a route's pattern or provider changes. The resolver is exact-match
//! and fail-closed: a route id with no `access_control_entities` row resolves
//! to [`DenyReason::UnknownEntity`](crate::authz::DenyReason::UnknownEntity)
//! before any rule runs.
//!
//! [`reconcile_gateway_entities_exact`] makes the catalog equal to the live
//! profile's routes: it registers the current ids and deletes the
//! `gateway_route` rows outside that set. Boot and the `admin config` CLI both
//! call it, keeping the catalog in step with the profile whether the operator
//! edited a route or simply started the app. Entities are registered
//! `default_included = false`: presence in the catalog never grants access on
//! its own — an explicit, role-scoped grant in `access_control_rules` still
//! has to allow the route, and the catalog is what a `gateway_route`
//! `entity_match` glob expands over.
//!
//! Deletion is by set difference, not by `source`. Provenance cannot carry it:
//! `access_control_entities.source` is rewritten on every conflicting upsert
//! (`ingestion/upsert.rs`), so a row this module wrote as `profile:<name>` is
//! relabelled `ingestion:access_control_config` the moment a roles.yaml rule
//! touches it. Filtering on `source` would therefore prune nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::repository::AccessControlRepository;
use super::types::EntityKind;
use crate::authz::error::{AuthzError, AuthzResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayReconcileReport {
    pub registered: usize,
    pub pruned: u64,
}

/// Refuses an empty `route_ids` rather than emptying the catalog. A profile
/// with no gateway routes is a plausible misread — a missing `gateway:` block,
/// a profile that failed to load — and wiping every route grant on that guess
/// is not a recoverable mistake. Callers with genuinely no routes have nothing
/// to reconcile and should not call this.
pub async fn reconcile_gateway_entities_exact(
    repo: &AccessControlRepository,
    route_ids: &[&str],
    source: &str,
) -> AuthzResult<GatewayReconcileReport> {
    if route_ids.is_empty() {
        return Err(AuthzError::Validation(
            "refusing to reconcile the gateway_route catalog against an empty route set — this \
             would delete every route entity and cascade away every route grant; check that the \
             profile actually declares a gateway"
                .to_owned(),
        ));
    }
    let pruned = repo
        .reconcile_entities(EntityKind::GatewayRoute, route_ids, false, source)
        .await?;
    Ok(GatewayReconcileReport {
        registered: route_ids.len(),
        pruned,
    })
}
