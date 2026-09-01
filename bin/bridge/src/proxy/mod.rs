//! Loopback inference proxy: server, forwarding, token cache, MCP probe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod comms;
pub mod dispatch;
pub mod forward;
pub mod heartbeat;
pub mod identity;
pub mod keepalive;
pub mod loopback;
pub mod mcp_probe;
pub mod peer;
pub mod portfile;
pub mod secret;
pub mod server;
pub mod session;
pub mod token_cache;
pub mod usage;

use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;

use crate::activity::ActivityLog;
use crate::config::{self, RuntimeConfig, SharedRuntimeConfig};
use crate::mcp_registry::McpRegistrySlot;
use crate::stdio::diag;
use identity::InstallId;
use peer::PeerIdentity;

pub use loopback::LoopbackEndpoint;
pub use server::{ProxyContext, ProxyStats, ServedProxy};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_mins(1);
pub use forward::REFRESH_THRESHOLD_SECS;
use session::SessionContext;
use token_cache::TokenCache;

pub const MAX_CANDIDATE_PORT: u16 = DEFAULT_PROXY_PORT + 9;

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

#[must_use]
pub fn candidate_ports(ours: &InstallId) -> Vec<u16> {
    let mut ports = Vec::with_capacity(11);
    if let Some(preferred) = portfile::preferred_port(ours) {
        ports.push(preferred);
    }
    for p in DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT {
        if !ports.contains(&p) {
            ports.push(p);
        }
    }
    // Why: port 0 is the last-resort OS-assigned ephemeral — it guarantees a
    // working proxy for this process, at the cost of a port that changes on
    // every restart.
    ports.push(0);
    ports
}

enum Bind {
    Listener(tokio::net::TcpListener),
    Sibling {
        port: u16,
        pid: u32,
        config_dir: String,
    },
    Exhausted,
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
            self.runtime.spawn(async move { cache.invalidate().await });
        }
        tracing::info!(target: "bridge::config", "runtime config swapped");
    }
}

fn bind_candidate(
    rt: &Handle,
    ours: &InstallId,
    tried: &mut Vec<u16>,
    last_error: &mut String,
) -> Bind {
    for port in candidate_ports(ours) {
        if port != 0 {
            match peer::probe_identity(port, ours) {
                PeerIdentity::Ours(who) => {
                    return Bind::Sibling {
                        port: who.port,
                        pid: who.pid,
                        config_dir: who.config_dir,
                    };
                },
                PeerIdentity::Foreign(who) => {
                    diag(&format!(
                        "proxy: port {port} is held by another {} install ({}); trying the next \
                         port",
                        crate::brand::brand().app_name,
                        who.config_dir
                    ));
                    tried.push(port);
                    continue;
                },
                PeerIdentity::Unknown => {
                    diag(&format!(
                        "proxy: port {port} is held by an unidentified listener; trying the next \
                         port"
                    ));
                    tried.push(port);
                    continue;
                },
                PeerIdentity::Unreachable => {},
            }
        }

        // Why: bind anyway even after a clean probe — another process can take
        // the port between the two calls.
        match rt.block_on(server::try_bind(port)) {
            Ok(l) => return Bind::Listener(l),
            Err(e) => {
                *last_error = e.to_string();
                tried.push(port);
            },
        }
    }
    Bind::Exhausted
}

fn persist_and_announce(port: u16, ours: &InstallId) {
    if (DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT).contains(&port) {
        if let Err(e) = portfile::write(port, ours) {
            tracing::warn!(error = %e, port, "could not record the bound proxy port");
        }
    } else {
        tracing::error!(
            port,
            "bound an ephemeral proxy port; it will change on every restart and client config \
             cannot track it",
        );
    }

    if port == DEFAULT_PROXY_PORT {
        diag(&format!("proxy: listening on localhost:{port}"));
        return;
    }

    let bin = crate::brand::brand().binary_name;
    diag(&format!(
        "proxy: port {DEFAULT_PROXY_PORT} was taken by another listener; listening on {port} \
         instead.\n       Client configs written for port {DEFAULT_PROXY_PORT} will be rejected \
         with 403 — run `{bin} install --apply` to repoint them, then restart the client."
    ));
    if port == 0 || !(DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT).contains(&port) {
        diag("       this port is ephemeral and will change on every restart.");
    }
}

fn portfile_port(ours: &InstallId) -> Option<u16> {
    let record = portfile::read(ours)?;
    match peer::probe_identity(record.port, ours) {
        // Why: down, or answering without identifying itself, still leaves the
        // record the best guess — the port is sticky by design.
        PeerIdentity::Ours(_) | PeerIdentity::Unreachable | PeerIdentity::Unknown => {
            Some(record.port)
        },
        PeerIdentity::Foreign(who) => {
            tracing::warn!(
                port = record.port,
                other = %who.config_dir,
                "our recorded proxy port is now held by another install",
            );
            None
        },
    }
}

async fn refresh_loop(cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        _ = cache.current(REFRESH_THRESHOLD_SECS).await;
    }
}
