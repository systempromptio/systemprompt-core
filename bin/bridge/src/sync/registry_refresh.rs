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
//! The same refresh precedes a host repair, whose policy writer reads the
//! envelope kept here. When the gateway cannot be reached the repair goes on
//! with the envelope last kept, which the writer re-verifies; any other
//! failure, a verification failure above all, stops the repair.
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
) -> Result<RegistryRefresh, SyncError> {
    if !host.profile_carries_managed_servers() {
        return Ok(RegistryRefresh::NotCarried);
    }
    refresh_or_keep_stored(bridge).await
}

/// What a refresh ahead of a profile write or repair produced.
#[derive(Debug)]
pub enum RegistryRefresh {
    NotCarried,
    Refreshed { servers: usize },
    StoredEnvelope(SyncError),
}

pub async fn refresh_or_keep_stored(bridge: &BridgeContext) -> Result<RegistryRefresh, SyncError> {
    match refresh_registry(bridge).await {
        Ok(servers) => Ok(RegistryRefresh::Refreshed { servers }),
        Err(error) if gateway_unreachable(&error) => {
            tracing::warn!(target: "bridge::sync", %error, "gateway unreachable; the repair uses the envelope last verified");
            Ok(RegistryRefresh::StoredEnvelope(error))
        },
        Err(error) => Err(error),
    }
}

#[must_use]
pub fn gateway_unreachable(error: &SyncError) -> bool {
    use crate::gateway::GatewayError;
    let transport = |e: &reqwest::Error| {
        e.status().is_none() && (e.is_connect() || e.is_timeout() || e.is_request())
    };
    match error {
        SyncError::Gateway(GatewayError::ManifestFetch(e) | GatewayError::PubkeyFetch(e)) => {
            transport(e)
        },
        SyncError::Authentication(chain) => !chain.is_terminal(),
        _ => false,
    }
}

pub async fn refresh_registry(bridge: &BridgeContext) -> Result<usize, SyncError> {
    let _operation = bridge.sync_lock.lock().await;
    let _across_processes = super::process_lock::acquire().await?;
    manifest::ensure_enabled(&config::load()?)?;
    let fetch = manifest::fetch_authenticated_manifest(&bridge.http, Freshness::Fresh).await?;
    let synced = manifest::verify_and_decode(&fetch, false, false).await?;
    super::acceptance::accept(
        &synced,
        fetch.client.base_url(),
        false,
        super::acceptance::CurrentVersion::for_run(
            fetch.revalidated,
            super::acceptance::CurrentVersion::Accepted,
        ),
    )?;
    let servers = apply::loopback::rewrite_loopback_urls(
        &synced.managed_mcp_servers,
        fetch.client.base_url(),
    );
    let meta_dir = apply::metadata_dir().map_err(|e| SyncError::ApplyFailed(Box::new(e)))?;
    super::ensure_not_superseded(fetch.client.base_url())?;
    let envelope_receipt = apply::write_envelope(
        &meta_dir,
        fetch.client.base_url(),
        &fetch.envelope,
        fetch.etag.as_deref(),
    )
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
