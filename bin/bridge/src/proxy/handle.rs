//! The proxy as owned by one process: its role, the loopback endpoint every
//! writer is handed, and the hot-swappable runtime config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use tokio::runtime::Handle;

use super::bind::{Bind, bind_candidate, persist_and_announce, portfile_port};
use super::identity::InstallId;
use super::peer::{self, PeerIdentity};
use super::session::SessionContext;
use super::token_cache::{AuthState, TokenCache};
use super::{
    DEFAULT_PROXY_PORT, LoopbackEndpoint, REFRESH_THRESHOLD_SECS, REFRESH_TICK, ServedProxy,
    portfile, secret, server,
};
use systemprompt_identifiers::SessionId;

use crate::activity::ActivityLog;
use crate::config::{self, RuntimeConfig, SharedRuntimeConfig};
use crate::mcp_registry::McpRegistrySlot;

/// What this process's relationship to the loopback port turned out to be.
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
    Failed {
        tried: Vec<u16>,
        last_error: String,
    },
}

/// The proxy as owned by one process: its role, the loopback endpoint every
/// writer is handed, and the hot-swappable runtime config.
///
/// Built once by the composition root (the bridge context) and
/// injected; nothing below the context reaches it ambiently.
pub struct ProxyHandle {
    role: ProxyRole,
    loopback: LoopbackEndpoint,
    deps: ProxyDeps,
    runtime: Handle,
    runtime_config: SharedRuntimeConfig,
    token_cache: Option<Arc<TokenCache>>,
    session_id: Option<SessionId>,
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

impl std::fmt::Debug for ProxyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyHandle")
            .field("role", &self.role)
            .field("loopback", &self.loopback)
            .finish_non_exhaustive()
    }
}

impl ProxyHandle {
    // Why: the outcome is recorded in `role` rather than returned as an error
    // because a GUI that lost the port race is still a useful GUI.
    #[must_use]
    pub fn serve(rt: &Handle, deps: ProxyDeps) -> Self {
        let runtime_config = config::shared_from_loaded();
        let mut tried = Vec::new();
        let mut last_error = "no candidate port could be bound".to_owned();

        let listener = match bind_candidate(rt, &deps.install_id, &mut tried, &mut last_error) {
            Bind::Listener(l) => l,
            Bind::Sibling {
                port,
                pid,
                config_dir,
            } => {
                return Self::not_serving(
                    rt,
                    deps,
                    runtime_config,
                    port,
                    ProxyRole::AlreadyRunning {
                        port,
                        pid,
                        config_dir,
                    },
                );
            },
            Bind::Exhausted => {
                return Self::failed(rt, deps, runtime_config, tried, last_error);
            },
        };

        let loopback_secret = match secret::proxy_init() {
            Ok(s) => s,
            Err(e) => return Self::failed(rt, deps, runtime_config, tried, e.to_string()),
        };
        let session_context = Arc::new(SessionContext::new());
        let session_id = session_context.session_id().clone();
        let token_cache = Arc::new(TokenCache::default_for_runtime(
            session_context.session_id().clone(),
            deps.http.clone(),
        ));
        let parts = server::ServerParts {
            loopback: loopback_secret.clone(),
            runtime_config: Arc::clone(&runtime_config),
            token_cache: Arc::clone(&token_cache),
            session: session_context,
            deps: deps.clone(),
        };
        let served = match server::start_with_listener(rt, listener, parts) {
            Ok(s) => s,
            Err(e) => return Self::failed(rt, deps, runtime_config, tried, e.to_string()),
        };

        persist_and_announce(served.port, &deps.install_id);
        rt.spawn(refresh_loop(Arc::clone(&token_cache)));

        Self {
            loopback: LoopbackEndpoint::new(served.port, Some(loopback_secret)),
            role: ProxyRole::Serving(served),
            deps,
            runtime: rt.clone(),
            runtime_config,
            token_cache: Some(token_cache),
            session_id: Some(session_id),
        }
    }

    // Why: `install --apply`, `sync` and `doctor` run beside a serving bridge
    // and must find its port, not race it — so nothing is bound here.
    #[must_use]
    pub fn attach(rt: &Handle, deps: ProxyDeps) -> Self {
        let port = portfile_port(&deps.install_id).unwrap_or(DEFAULT_PROXY_PORT);
        Self::not_serving(
            rt,
            deps,
            config::shared_from_loaded(),
            port,
            ProxyRole::Attached,
        )
    }

    fn failed(
        rt: &Handle,
        deps: ProxyDeps,
        runtime_config: SharedRuntimeConfig,
        tried: Vec<u16>,
        last_error: String,
    ) -> Self {
        let port = portfile_port(&deps.install_id).unwrap_or(DEFAULT_PROXY_PORT);
        Self::not_serving(
            rt,
            deps,
            runtime_config,
            port,
            ProxyRole::Failed { tried, last_error },
        )
    }

    fn not_serving(
        rt: &Handle,
        deps: ProxyDeps,
        runtime_config: SharedRuntimeConfig,
        port: u16,
        role: ProxyRole,
    ) -> Self {
        Self {
            role,
            loopback: LoopbackEndpoint::new(port, None),
            deps,
            runtime: rt.clone(),
            runtime_config,
            token_cache: None,
            session_id: None,
        }
    }

    #[must_use]
    pub const fn install_id(&self) -> &InstallId {
        &self.deps.install_id
    }

    #[must_use]
    pub fn peer(&self) -> PeerIdentity {
        peer::probe_identity(self.port(), &self.deps.install_id)
    }

    pub fn forget_recorded_port(&self) {
        portfile::clear(&self.deps.install_id);
    }

    #[must_use]
    pub const fn role(&self) -> &ProxyRole {
        &self.role
    }

    #[must_use]
    pub const fn served(&self) -> Option<&ServedProxy> {
        match &self.role {
            ProxyRole::Serving(s) => Some(s),
            ProxyRole::Attached | ProxyRole::AlreadyRunning { .. } | ProxyRole::Failed { .. } => {
                None
            },
        }
    }

    #[must_use]
    pub const fn is_serving(&self) -> bool {
        self.served().is_some()
    }

    #[must_use]
    pub const fn loopback(&self) -> &LoopbackEndpoint {
        &self.loopback
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.loopback.port()
    }

    #[must_use]
    pub const fn runtime_config(&self) -> &SharedRuntimeConfig {
        &self.runtime_config
    }

    pub fn reload_runtime_config(&self) {
        self.runtime_config
            .store(Arc::new(RuntimeConfig::from_loaded()));
        if let Some(cache) = &self.token_cache {
            let cache = Arc::clone(cache);
            self.runtime.spawn(async move { cache.reset().await });
        }
        tracing::info!(target: "bridge::config", "runtime config swapped");
    }

    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    #[must_use]
    pub fn auth_state(&self) -> Option<tokio::sync::watch::Receiver<AuthState>> {
        self.token_cache.as_ref().map(|cache| cache.auth_state())
    }
}

// Why: the tick renews a token that is about to expire; it never acquires one.
// Acquisition is request-driven, and on a signed-out install a minting tick
// would fail — and, through the session provider, prompt — every minute.
async fn refresh_loop(cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(e) = cache.refresh_if_cached(REFRESH_THRESHOLD_SECS).await {
            tracing::debug!(error = %e, "token refresh tick did not renew");
        }
    }
}
