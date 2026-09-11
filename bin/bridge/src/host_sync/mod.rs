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

mod error;

pub use error::{ApplyError, TomlError};

/// A host sync that completed but could not do everything it exists to do —
/// the run is not partial, yet the operator has something to act on.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostWarning {
    pub host_id: String,
    pub message: String,
}

/// Warnings a host sync raises without failing. Shared by every emitter of one
/// run; drained into `SyncSummary.host_warnings` when the run ends.
#[derive(Debug, Default)]
pub struct HostWarnings(std::sync::Mutex<Vec<HostWarning>>);

impl HostWarnings {
    #[must_use]
    pub const fn new() -> Self {
        Self(std::sync::Mutex::new(Vec::new()))
    }

    pub fn push(&self, host_id: &str, message: impl Into<String>) {
        let warning = HostWarning {
            host_id: host_id.to_owned(),
            message: message.into(),
        };
        tracing::warn!(
            target: "bridge::sync::host",
            host = host_id,
            warning = %warning.message,
            "host sync warning"
        );
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(warning);
    }

    #[must_use]
    pub fn drain(&self) -> Vec<HostWarning> {
        std::mem::take(
            &mut *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

#[derive(Debug)]
pub struct HostSyncCtx<'a> {
    pub policy_store: &'a crate::config::store::PolicyStore,
    pub warnings: &'a HostWarnings,
    pub manifest: &'a SignedManifest,
    pub org_plugins_root: &'a Path,
    pub plugin_mcp_servers: &'a std::collections::BTreeMap<String, Vec<String>>,
    pub client: &'a GatewayClient,
    pub bearer: &'a str,
    pub loopback: &'a crate::proxy::LoopbackEndpoint,
    pub mcp_registry: &'a crate::mcp_registry::McpRegistry,
}

#[async_trait]
pub trait HostSync: std::any::Any + Send + Sync + 'static {
    fn host_id(&self) -> &'static str;
    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError>;
    fn clear(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError>;
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

#[macro_export]
macro_rules! register_host_sync {
    ($e:expr, priority = $p:expr $(,)?) => {
        ::inventory::submit! {
            $crate::host_sync::HostSyncRegistration { emitter: &$e, priority: $p }
        }
    };
    ($e:expr $(,)?) => {
        $crate::register_host_sync!($e, priority = 0);
    };
}


static REGISTRY: LazyLock<Vec<&'static dyn HostSync>> = LazyLock::new(|| {
    let mut regs: Vec<&'static HostSyncRegistration> =
        inventory::iter::<HostSyncRegistration>().collect();
    regs.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.emitter.host_id().cmp(b.emitter.host_id()))
    });
    let mut seen: std::collections::BTreeSet<std::any::TypeId> = std::collections::BTreeSet::new();
    let mut v: Vec<&'static dyn HostSync> = regs
        .into_iter()
        .filter(|r| seen.insert(r.emitter.type_id()))
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
