//! Per-host sync trait + central dispatcher.
//!
//! Every host integration that writes to the local filesystem on each manifest
//! apply implements [`HostSync`]. The dispatcher in `sync::apply::mod` walks
//! [`registry()`], decides per-host whether to call `apply` or `clear` based on
//! the manifest's `enabled_hosts` list, and uniformly logs the outcome — so
//! emitter authors never re-implement the toggle-and-cleanup gate.

use std::path::Path;
use std::sync::LazyLock;

use crate::gateway::manifest::SignedManifest;

use super::apply::ApplyError;

pub struct HostSyncCtx<'a> {
    pub manifest: &'a SignedManifest,
    pub org_plugins_root: &'a Path,
}

pub trait HostSync: Send + Sync + 'static {
    fn host_id(&self) -> &'static str;
    fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError>;
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
        Err(e) => tracing::warn!(
            target: "bridge::sync::host",
            host = host_id,
            action,
            error = %e,
            "host sync failed (non-fatal)"
        ),
    }
}
