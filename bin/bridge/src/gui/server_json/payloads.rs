//! Builds the wire sub-payloads from the GUI's own state types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::state::{
    AppStateSnapshot, CachedToken, CatalogFreshness, GatewayStatus, InferenceCatalog,
    VerifiedIdentity,
};
use crate::wire::catalog::{CatalogFreshnessPayload, InferenceCatalogPayload};
pub(super) use crate::wire::payloads::{
    CachedTokenPayload, GatewayStatusPayload, McpServerAuthPayload, ProxyStatsPayload,
    UpdatePayload, ValidationPayload, VerifiedIdentityPayload,
};

pub(super) fn mcp_servers_payload(snap: &AppStateSnapshot) -> Vec<McpServerAuthPayload<'_>> {
    snap.mcp_auth
        .iter()
        .map(|server| McpServerAuthPayload {
            desktop_tools_unlisted: snap.mcp_desktop_unlisted.contains(&server.id),
            ..McpServerAuthPayload::from(server)
        })
        .collect()
}

pub(super) const fn cached_token_payload(t: &CachedToken) -> CachedTokenPayload {
    CachedTokenPayload {
        ttl_seconds: t.ttl_seconds,
        length: t.length,
    }
}

pub(super) const fn gateway_status_payload(s: &GatewayStatus) -> GatewayStatusPayload<'_> {
    let (latency_ms, reason) = match s {
        GatewayStatus::Reachable { latency_ms } => (Some(*latency_ms), None),
        GatewayStatus::Unreachable { reason } => (None, Some(reason.as_str())),
        GatewayStatus::Unknown | GatewayStatus::Probing => (None, None),
    };
    GatewayStatusPayload {
        verdict: s.verdict(),
        settled: s.settled(),
        latency_ms,
        reason,
    }
}

pub(super) fn verified_identity_payload(v: &VerifiedIdentity) -> VerifiedIdentityPayload<'_> {
    VerifiedIdentityPayload {
        email: v.email.as_deref(),
        user_id: v.user_id.as_ref(),
        tenant_id: v.tenant_id.as_ref(),
        exp_unix: v.exp_unix,
        verified_at_unix: v.verified_at_unix,
    }
}

pub(super) fn inference_catalog_payload(catalog: &InferenceCatalog) -> InferenceCatalogPayload {
    let freshness = match &catalog.freshness {
        CatalogFreshness::Unknown => CatalogFreshnessPayload::Unknown,
        CatalogFreshness::Fresh => CatalogFreshnessPayload::Fresh,
        CatalogFreshness::Stale { since, error } => CatalogFreshnessPayload::Stale {
            since_unix: *since,
            error: error.clone(),
        },
    };
    InferenceCatalogPayload::new(
        crate::gateway::model_view::catalog_models(&catalog.providers),
        freshness,
        catalog.fetched_at_unix,
    )
}
