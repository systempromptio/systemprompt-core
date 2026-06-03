//! Per-host sync trait + central dispatcher.
//!
//! Each host integration implements [`HostSync`]. The dispatcher walks
//! [`registry()`] and, per-host, calls `apply` or `clear` based on the
//! manifest's `enabled_hosts` list, logging the outcome uniformly.

use async_trait::async_trait;
use std::path::Path;
use std::sync::LazyLock;

use crate::gateway::GatewayClient;
use crate::gateway::manifest::SignedManifest;

use super::apply::ApplyError;

#[derive(Debug)]
pub struct HostSyncCtx<'a> {
    pub manifest: &'a SignedManifest,
    pub org_plugins_root: &'a Path,
    pub client: &'a GatewayClient,
    pub bearer: &'a str,
}

// `dyn HostSync` is needed by the static registry, so this trait must be
// dyn-compatible — `#[async_trait]` boxes the future to satisfy that.
#[async_trait]
pub trait HostSync: Send + Sync + 'static {
    fn host_id(&self) -> &'static str;
    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError>;
    fn clear(&self) -> Result<(), ApplyError>;
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
const DESKTOP_EMITTERS: &[&'static dyn HostSync] = &[&crate::install::mdm::ClaudeDesktopMdmSync];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DESKTOP_EMITTERS: &[&'static dyn HostSync] = &[];

static REGISTRY: LazyLock<Vec<&'static dyn HostSync>> = LazyLock::new(|| {
    let mut v: Vec<&'static dyn HostSync> = Vec::new();
    v.push(&crate::sync::apply::synthetic_plugin::ClaudeCodePluginSync);
    v.extend_from_slice(DESKTOP_EMITTERS);
    v.push(&crate::integration::cowork_plugins::CoworkSync);
    v.push(&crate::integration::codex_cli::CodexCliSync);
    v
});

pub fn registry() -> &'static [&'static dyn HostSync] {
    REGISTRY.as_slice()
}

pub fn log_outcome(host_id: &str, enabled: bool, outcome: Result<(), ApplyError>) {
    let action = if enabled { "apply" } else { "clear" };
    match outcome {
        Ok(()) => tracing::info!(
            target: "bridge::sync::host",
            host = host_id,
            action,
            "host sync ok"
        ),
        Err(e) => tracing::error!(
            target: "bridge::sync::host",
            host = host_id,
            action,
            error = %e,
            "host sync failed — partial sync; see SyncSummary.host_failures"
        ),
    }
}
