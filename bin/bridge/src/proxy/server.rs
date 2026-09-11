//! Loopback proxy HTTP server: listener binding and per-request dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::runtime::Handle;

use crate::config::{RuntimeConfig, SharedRuntimeConfig};
use crate::ids::{LoopbackSecret, ProxySecret};
use crate::proxy::session::SessionContext;
use crate::proxy::token_cache::TokenCache;
use crate::proxy::{dispatch, heartbeat};

/// A proxy this process bound and is serving: the port it actually got and
/// the counters the Status pane reads.
#[derive(Clone, Debug)]
pub struct ServedProxy {
    pub(crate) tasks: Arc<crate::tasks::TaskOwner>,
    pub port: u16,
    pub stats: Arc<ProxyStats>,
    shutdown: tokio::sync::watch::Sender<bool>,
    drained: Arc<AtomicBool>,
}

pub const DRAIN_DEADLINE: Duration = Duration::from_secs(5);

impl ServedProxy {
    // Why: bounded rather than unconditional — a streaming response can outlive
    // any deadline, and the caller, a restart, has to make progress. Returns
    // whether the listener actually drained within the deadline.
    pub fn drain(&self, deadline: Duration) -> bool {
        if self.shutdown.send(true).is_err() {
            return self.drained.load(Ordering::Relaxed);
        }
        let started = std::time::Instant::now();
        while started.elapsed() < deadline {
            if self.drained.load(Ordering::Relaxed) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        self.drained.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Default)]
pub struct ProxyStats {
    pub forwarded_total: AtomicU64,
    pub last_forwarded_at_unix: AtomicU64,
    pub last_status: AtomicU64,
    pub last_latency_ms: AtomicU64,
    pub messages_total: AtomicU64,
    pub tokens_in_total: AtomicU64,
    pub tokens_out_total: AtomicU64,
}

#[derive(Clone)]
#[expect(
    missing_debug_implementations,
    reason = "holds a TokenCache whose RefreshFn (Box<dyn Fn>) cannot derive Debug"
)]
pub struct ProxyContext {
    pub runtime_config: SharedRuntimeConfig,
    pub secret: Arc<ProxySecret>,
    pub stats: Arc<ProxyStats>,
    pub client: reqwest::Client,
    pub token_cache: Arc<TokenCache>,
    pub session: Arc<SessionContext>,
    pub port: u16,
    pub started_at_unix: u64,
    pub deps: super::ProxyDeps,
}

impl ProxyContext {
    pub fn snapshot(&self) -> Arc<RuntimeConfig> {
        self.runtime_config.load_full()
    }
}

/// What a proxy server is built from, besides the listener.
#[expect(
    missing_debug_implementations,
    reason = "holds a TokenCache whose RefreshFn (Box<dyn Fn>) cannot derive Debug"
)]
pub struct ServerParts {
    pub loopback: LoopbackSecret,
    pub runtime_config: SharedRuntimeConfig,
    pub token_cache: Arc<TokenCache>,
    pub session: Arc<SessionContext>,
    pub deps: super::ProxyDeps,
}

pub fn start(rt: &Handle, port: u16, parts: ServerParts) -> std::io::Result<ServedProxy> {
    let listener = rt.block_on(try_bind(port))?;
    start_with_listener(rt, listener, parts)
}

pub fn start_with_listener(
    rt: &Handle,
    listener: TcpListener,
    parts: ServerParts,
) -> std::io::Result<ServedProxy> {
    let ServerParts {
        loopback,
        runtime_config,
        token_cache,
        session,
        deps,
    } = parts;
    let bound_port = listener.local_addr()?.port();

    let proxy_secret = ProxySecret::new(loopback.into_inner());
    let stats = Arc::new(ProxyStats::default());

    let client = build_upstream_client()?;

    let ctx = ProxyContext {
        runtime_config: Arc::clone(&runtime_config),
        secret: Arc::new(proxy_secret),
        stats: Arc::clone(&stats),
        client: client.clone(),
        token_cache: Arc::clone(&token_cache),
        session: Arc::clone(&session),
        port: bound_port,
        started_at_unix: crate::proxy::identity::now_unix(),
        deps,
    };

    let (shutdown, shutdown_rx) = tokio::sync::watch::channel(false);
    let drained = Arc::new(AtomicBool::new(false));

    let tasks = Arc::new(crate::tasks::TaskOwner::new(rt, ctx.deps.activity.clone()));
    tasks.spawn(run_listener(
        listener,
        ctx,
        shutdown_rx,
        Arc::clone(&drained),
    ));
    tasks.spawn(heartbeat::run_loop(
        Arc::clone(&runtime_config),
        Arc::clone(&token_cache),
        session,
        Arc::clone(&stats),
        client.clone(),
    ));
    tasks.spawn(crate::proxy::comms::run_loop(
        runtime_config,
        token_cache,
        client,
    ));

    Ok(ServedProxy {
        tasks,
        port: bound_port,
        stats,
        shutdown,
        drained,
    })
}

// Why: the WSL localhost relay on Windows silently discards an idle keep-alive
// socket; a request written to one afterwards is retransmitted for ~30 s and
// then aborted, which the host app reads as the server being unreachable.
// Reconnecting to a loopback gateway is cheap, so idle sockets are dropped well
// before the relay does it for us. The replay in `forward` covers the window
// this cannot.
const UPSTREAM_POOL_IDLE: Duration = Duration::from_secs(15);

fn build_upstream_client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(crate::gateway::Ipv4FirstResolver))
        .pool_max_idle_per_host(16)
        .pool_idle_timeout(UPSTREAM_POOL_IDLE)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_mins(10))
        .build()
        .map_err(|e| std::io::Error::other(format!("upstream client build failed: {e}")))
}

async fn run_listener(
    listener: TcpListener,
    ctx: ProxyContext,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    drained: Arc<AtomicBool>,
) {
    let mut connections = tokio::task::JoinSet::new();
    loop {
        let accepted = tokio::select! {
            _ = shutdown.changed() => break,
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(e)) = result { ctx.deps.activity.append_error(format!("proxy connection task: {e}")); }
                continue;
            },
            result = listener.accept() => result,
        };
        let (stream, peer) = match accepted {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    target: "systemprompt_bridge::proxy",
                    error = %e,
                    "proxy accept failed"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            },
        };
        if let Err(e) = stream.set_nodelay(true) {
            tracing::error!(error = %e, "proxy connection setup failed");
            continue;
        }
        let conn_ctx = ctx.clone();
        connections.spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| dispatch::handle_request(req, conn_ctx.clone(), peer));
            if let Err(e) = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(io, svc)
                .await
            {
                let msg = e.to_string();
                if !msg.contains("closed") && !msg.contains("connection") {
                    tracing::warn!(
                        target: "systemprompt_bridge::proxy",
                        error = %msg,
                        "proxy connection error"
                    );
                }
            }
        });
    }

    // Why: dropping the listener is what frees the port for a successor
    // process, so it happens before the in-flight wait rather than after it.
    drop(listener);
    let all_finished = async { while connections.join_next().await.is_some() {} };
    if tokio::time::timeout(DRAIN_DEADLINE, all_finished)
        .await
        .is_ok()
    {
        drained.store(true, Ordering::Relaxed);
    } else {
        ctx.deps
            .activity
            .append_error("proxy drain: connections were still open at the deadline".to_owned());
    }
}

pub async fn try_bind(port: u16) -> std::io::Result<TcpListener> {
    let v4: SocketAddr = SocketAddr::from(([127u8, 0, 0, 1], port));
    if let Ok(l) = TcpListener::bind(v4).await {
        return Ok(l);
    }
    let v6: SocketAddr = SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 1], port));
    TcpListener::bind(v6).await
}
