//! Per-host sync trait + central dispatcher. The dispatcher walks
//! [`registry()`] and calls `apply` or `clear` per the manifest's
//! `enabled_hosts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::BearerToken;
use async_trait::async_trait;
use std::path::Path;
use std::sync::LazyLock;
use systemprompt_models::bridge::host::HostKind;

use crate::gateway::GatewayClient;
use crate::gateway::manifest::SignedManifest;

mod error;
mod hooks;
pub(crate) mod hooks_schema;
mod scope;

pub use error::{ApplyError, ForeignShape, TomlError};
pub use hooks::stamp_hooks_file;
pub use scope::{UnknownWarningScope, WarningScope};

/// What a host warning is about. Control flow (the Cowork re-sync tick, the
/// health verdict) reads this, never the message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum HostWarningKind {
    CoworkSessionMissing,
    PluginDependencies,
    EvidenceUnacknowledged,
    PermissionRules,
    ToolCatalog,
    NodePackages,
    Manifest,
    PolicyWriter,
}

/// A host sync that completed but could not do everything it exists to do —
/// the run is not partial, yet the operator has something to act on.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostWarning {
    pub kind: HostWarningKind,
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub host_id: WarningScope,
    pub message: String,
}

impl HostWarning {
    #[must_use]
    pub fn raise(
        kind: HostWarningKind,
        host_id: impl Into<WarningScope>,
        message: impl Into<String>,
    ) -> Self {
        let warning = Self {
            kind,
            host_id: host_id.into(),
            message: message.into(),
        };
        tracing::warn!(
            target: "bridge::sync::host",
            host = %warning.host_id,
            kind = ?warning.kind,
            warning = %warning.message,
            "host sync warning"
        );
        warning
    }
}

/// What a host sync that completed reports back.
///
/// It carries the warnings the sync raised without failing and is the return
/// value of [`HostSync::apply`], so a degraded sync cannot report success
/// without the caller seeing why.
#[derive(Debug, Default)]
pub struct HostSyncReport {
    pub warnings: Vec<HostWarning>,
}

impl HostSyncReport {
    #[must_use]
    pub fn ok() -> Self {
        Self::default()
    }

    pub fn warn(&mut self, kind: HostWarningKind, host_id: HostKind, message: impl Into<String>) {
        self.warnings
            .push(HostWarning::raise(kind, host_id, message));
    }

    pub fn merge(&mut self, other: Self) {
        self.warnings.extend(other.warnings);
    }
}

#[derive(Debug)]
pub struct HostSyncCtx<'a> {
    pub policy_store: &'a crate::config::store::PolicyStore,
    pub manifest: &'a SignedManifest,
    pub org_plugins_root: &'a Path,
    pub plugin_mcp_servers: &'a std::collections::BTreeMap<String, Vec<String>>,
    pub client: &'a GatewayClient,
    pub bearer: &'a BearerToken,
    pub loopback: &'a crate::proxy::LoopbackEndpoint,
    pub mcp_registry: &'a crate::mcp_registry::McpRegistry,
    pub start_menu: &'a crate::probe_cache::StartMenuCache,
}

/// One emitter per host integration.
///
/// `#[async_trait]` because emitters are collected through `inventory` as
/// `&'static dyn HostSync`. Several emitters may share a `host_id` (the
/// enable gate); `emitter_id` names the emitter itself so failures from
/// siblings stay distinguishable.
#[async_trait]
pub trait HostSync: std::any::Any + Send + Sync + 'static {
    fn host_id(&self) -> HostKind;
    fn emitter_id(&self) -> &'static str {
        self.host_id().as_str()
    }
    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError>;
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
            .then_with(|| a.emitter.host_id().cmp(&b.emitter.host_id()))
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

pub fn log_outcome(emitter: &dyn HostSync, enabled: bool, outcome: Result<(), &ApplyError>) {
    let action = if enabled { "apply" } else { "clear" };
    match outcome {
        Ok(()) => tracing::info!(
            target: "bridge::sync::host",
            host = %emitter.host_id(),
            emitter = emitter.emitter_id(),
            action,
            "host sync ok"
        ),
        Err(e) => tracing::error!(
            target: "bridge::sync::host",
            host = %emitter.host_id(),
            emitter = emitter.emitter_id(),
            action,
            error = %e,
            "host sync failed — partial sync; see SyncSummary.host_failures"
        ),
    }
}
