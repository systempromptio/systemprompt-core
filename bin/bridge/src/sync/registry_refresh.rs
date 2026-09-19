//! Refresh the managed MCP registry from the gateway without a full sync.
//!
//! A host profile names the servers the user may reach, and until now it
//! took them from the registry the last sync published — which could be a
//! minute behind the gateway (its per-user memo) and further behind the
//! user, who has just linked a connector and pressed Update. This fetches
//! the manifest fresh, verifies it exactly as a sync would, and publishes
//! the servers to the registry and its on-disk fragment, so the profile
//! written next carries the gateway's current truth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use super::{apply, manifest};
use crate::config;
use crate::context::BridgeContext;
use crate::gateway::Freshness;

// Why: only a host whose profile itself names the managed servers (Claude
// Desktop's policy) is written from the registry; every other host reaches
// them through the synced org-plugins, so a refresh would be a wasted fetch.
pub async fn refresh_registry_for(
    bridge: &BridgeContext,
    host: &dyn crate::integration::HostApp,
) -> Result<Option<usize>, SyncError> {
    if !host.profile_carries_managed_servers() {
        return Ok(None);
    }
    refresh_registry(bridge).await.map(Some)
}

pub async fn refresh_registry(bridge: &BridgeContext) -> Result<usize, SyncError> {
    let allow_tofu = matches!(
        config::pinned_pubkey_state(),
        Ok(config::PinnedPubkeyState::Unpinned)
    );
    let fetch = manifest::fetch_authenticated_manifest(&bridge.http, Freshness::Fresh).await?;
    let synced = manifest::verify_and_decode(&fetch, false, allow_tofu).await?;
    let servers = apply::loopback::rewrite_loopback_urls(
        &synced.managed_mcp_servers,
        fetch.client.base_url(),
    );
    let meta_dir = apply::metadata_dir().map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    let envelope_receipt =
        apply::write_envelope(&meta_dir, fetch.client.base_url(), &fetch.envelope)
            .map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    let receipt = apply::write_mcp_servers(&meta_dir, fetch.client.base_url(), &servers)
        .map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    crate::mcp_registry::publish(&bridge.mcp_registry, &servers);
    tracing::info!(
        target: "bridge::sync",
        servers = servers.len(),
        fragment = %receipt.path().display(),
        envelope = %envelope_receipt.path().display(),
        "managed MCP registry refreshed from a fresh manifest"
    );
    Ok(servers.len())
}
