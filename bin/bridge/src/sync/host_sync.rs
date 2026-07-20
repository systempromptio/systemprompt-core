//! Per-host sync trait + central dispatcher. The dispatcher walks
//! [`registry()`] and calls `apply` or `clear` per the manifest's
//! `enabled_hosts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    /// MCP server names per plugin id, recovered from each bundle's stripped
    /// `.mcp.json`; emitters re-project them through the bridge proxy.
    pub plugin_mcp_servers: &'a std::collections::BTreeMap<String, Vec<String>>,
    pub client: &'a GatewayClient,
    pub bearer: &'a str,
}

// `#[async_trait]`: the static registry needs `dyn HostSync`.
#[async_trait]
pub trait HostSync: Send + Sync + 'static {
    fn host_id(&self) -> &'static str;
    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError>;
    fn clear(&self) -> Result<(), ApplyError>;
}

#[derive(Clone, Copy)]
pub struct HostSyncRegistration {
    pub emitter: &'static dyn HostSync,
    pub priority: i32,
}

impl std::fmt::Debug for HostSyncRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostSyncRegistration")
            .field("host_id", &self.emitter.host_id())
            .field("priority", &self.priority)
            .finish()
    }
}

inventory::collect!(HostSyncRegistration);

/// Register a [`HostSync`] emitter into the compile-time sync registry.
///
/// Pass a zero-sized `'static` emitter value. An optional `priority = N`
/// (default 0) lets a registration shadow a built-in sharing the same
/// `host_id()`.
#[macro_export]
macro_rules! register_host_sync {
    ($e:expr, priority = $p:expr $(,)?) => {
        ::inventory::submit! {
            $crate::sync::host_sync::HostSyncRegistration { emitter: &$e, priority: $p }
        }
    };
    ($e:expr $(,)?) => {
        $crate::register_host_sync!($e, priority = 0);
    };
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
register_host_sync!(crate::install::mdm::ClaudeDesktopMdmSync);
register_host_sync!(crate::integration::cowork_plugins::CoworkSync);
register_host_sync!(crate::integration::cowork_artifacts::CoworkArtifactsSync);
register_host_sync!(crate::integration::codex_cli::CodexCliSync);
register_host_sync!(crate::integration::claude_code_cli::ClaudeCodeCliSync);

static REGISTRY: LazyLock<Vec<&'static dyn HostSync>> = LazyLock::new(|| {
    let mut regs: Vec<&'static HostSyncRegistration> =
        inventory::iter::<HostSyncRegistration>().collect();
    regs.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.emitter.host_id().cmp(b.emitter.host_id()))
    });
    let mut seen: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    let mut v: Vec<&'static dyn HostSync> = regs
        .into_iter()
        .filter(|r| seen.insert(r.emitter.host_id()))
        .map(|r| r.emitter)
        .collect();
    v.sort_by_key(|s| s.host_id());
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
