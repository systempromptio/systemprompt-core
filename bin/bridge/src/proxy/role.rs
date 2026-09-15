//! What a process's relationship to the loopback port turned out to be, and
//! the services a proxy shares with the rest of the process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::ServedProxy;
use super::identity::InstallId;
use crate::activity::ActivityLog;
use crate::config::ConfigReadError;
use crate::mcp_registry::McpRegistrySlot;

/// What a process's relationship to the loopback port turned out to be.
///
/// `Option<&ServedProxy>` could not express the middle cases: a sibling window
/// of this same install already serving the port is a success for the caller
/// even though this process bound nothing, and a process that never tried to
/// bind (`install`, `sync`, `doctor`) is not a failure either.
#[derive(Debug)]
pub enum ProxyRole {
    Serving(ServedProxy),
    Attached,
    AlreadyRunning {
        port: u16,
        pid: u32,
        config_dir: String,
    },
    Failed(ProxyFailure),
}

/// Why this process is not serving the loopback port.
///
/// A proxy that cannot read its config or its secret never binds: serving
/// inference against a default gateway with whatever credential is on disk
/// is worse than not serving.
#[derive(Debug, thiserror::Error)]
pub enum ProxyFailure {
    #[error("config unreadable; refusing to serve against defaults: {0}")]
    Config(#[source] ConfigReadError),
    #[error("loopback secret: {0}")]
    LoopbackSecret(#[source] std::io::Error),
    #[error("no candidate port could be bound (tried {tried:?}): {last_error}")]
    Bind { tried: Vec<u16>, last_error: String },
    #[error("proxy server start: {0}")]
    Server(#[source] std::io::Error),
}

impl ProxyFailure {
    #[must_use]
    pub fn tried_ports(&self) -> &[u16] {
        match self {
            Self::Bind { tried, .. } => tried,
            Self::Config(_) | Self::LoopbackSecret(_) | Self::Server(_) => &[],
        }
    }
}

/// The services a proxy shares with the rest of the process: who this install
/// is, the managed-MCP routes, and the activity log its requests write to.
#[derive(Clone)]
pub struct ProxyDeps {
    pub install_id: InstallId,
    pub mcp_registry: Arc<McpRegistrySlot>,
    pub activity: ActivityLog,
    pub http: reqwest::Client,
    pub plugin_tokens: Arc<crate::auth::plugin_oauth::PluginTokenCache>,
}

impl std::fmt::Debug for ProxyDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyDeps")
            .field("install_id", &self.install_id)
            .finish_non_exhaustive()
    }
}
