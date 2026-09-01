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
pub mod mcp_probe;
pub mod peer;
pub mod portfile;
mod runtime;
pub mod secret;
pub mod server;
pub mod session;
pub mod token_cache;
pub mod usage;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::stdio::diag;
use peer::PeerIdentity;

use runtime::runtime;
pub use runtime::{block_on, reload_runtime_config, runtime_config, runtime_handle};
pub use server::{ProxyHandle, ProxyStats};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_mins(1);
pub use forward::REFRESH_THRESHOLD_SECS;
use session::SessionContext;
use token_cache::TokenCache;

static HANDLE: OnceLock<ProxyHandle> = OnceLock::new();
static RESOLVED_PORT: OnceLock<u16> = OnceLock::new();

pub const MAX_CANDIDATE_PORT: u16 = DEFAULT_PROXY_PORT + 9;

/// What happened when this process tried to own a loopback port.
///
/// `Option<&ProxyHandle>` could not express the middle case: our own proxy is
/// already serving, which is a success for the caller even though this process
/// bound nothing.
#[derive(Debug)]
pub enum StartOutcome {
    Started(&'static ProxyHandle),
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

impl StartOutcome {
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        match self {
            Self::Started(h) => Some(h.port),
            Self::AlreadyRunning { port, .. } => Some(*port),
            Self::Failed { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.port().is_some()
    }
}

#[must_use]
pub fn candidate_ports() -> Vec<u16> {
    let mut ports = Vec::with_capacity(11);
    if let Some(preferred) = portfile::preferred_port() {
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

pub fn start_default() -> StartOutcome {
    if let Some(h) = HANDLE.get() {
        return StartOutcome::Started(h);
    }
    crate::mcp_registry::rehydrate_from_disk();
    let rt = match runtime() {
        Ok(rt) => rt,
        Err(e) => {
            diag(&format!("proxy: tokio runtime build failed: {e}"));
            return StartOutcome::Failed {
                tried: Vec::new(),
                last_error: format!("tokio runtime build failed: {e}"),
            };
        },
    };

    let mut tried = Vec::new();
    let mut last_error = "no candidate port could be bound".to_owned();
    let mut listener = None;

    for port in candidate_ports() {
        if port != 0 {
            match peer::probe_identity(port) {
                PeerIdentity::Ours(who) => {
                    return StartOutcome::AlreadyRunning {
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
            Ok(l) => {
                listener = Some(l);
                break;
            },
            Err(e) => {
                last_error = e.to_string();
                tried.push(port);
            },
        }
    }

    let Some(listener) = listener else {
        return StartOutcome::Failed { tried, last_error };
    };

    let shared = runtime_config();
    let session_context = Arc::new(SessionContext::new());
    let token_cache = Arc::new(TokenCache::default_for_runtime(
        session_context.session_id().clone(),
    ));
    runtime::remember_token_cache(&token_cache);

    let handle = match server::start_with_listener(
        rt,
        listener,
        Arc::clone(&shared),
        Arc::clone(&token_cache),
        Arc::clone(&session_context),
    ) {
        Ok(h) => h,
        Err(e) => {
            return StartOutcome::Failed {
                tried,
                last_error: e.to_string(),
            };
        },
    };

    persist_and_announce(handle.port);
    rt.spawn(refresh_loop(token_cache));

    _ = HANDLE.set(handle);
    HANDLE
        .get()
        .map_or_else(|| unreachable!("handle set above"), StartOutcome::Started)
}

fn persist_and_announce(port: u16) {
    if (DEFAULT_PROXY_PORT..=MAX_CANDIDATE_PORT).contains(&port) {
        if let Err(e) = portfile::write(port) {
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

pub fn handle() -> Option<&'static ProxyHandle> {
    HANDLE.get()
}

#[must_use]
pub fn mcp_url(slug: &str) -> String {
    format!("{}/mcp/{slug}", loopback_origin())
}

#[must_use]
pub fn resolved_port() -> u16 {
    if let Some(h) = handle() {
        return h.port;
    }
    // Why: cached because `mcp_url` is called in loops and must not re-probe
    // each time.
    *RESOLVED_PORT.get_or_init(|| portfile_port().unwrap_or(DEFAULT_PROXY_PORT))
}

fn portfile_port() -> Option<u16> {
    let record = portfile::read()?;
    match peer::probe_identity(record.port) {
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

#[must_use]
pub fn loopback_origin() -> String {
    format!("http://127.0.0.1:{}", resolved_port())
}

pub fn loopback_bearer() -> std::io::Result<String> {
    secret::proxy_init().map(|s| format!("Bearer {}", s.as_str()))
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
