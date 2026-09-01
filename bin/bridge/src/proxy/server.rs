//! Loopback proxy HTTP server: listener binding and per-request dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
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
    pub port: u16,
    pub stats: Arc<ProxyStats>,
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
    };

    rt.spawn(run_listener(listener, ctx));
    rt.spawn(heartbeat::run_loop(
        Arc::clone(&runtime_config),
        Arc::clone(&token_cache),
        session,
        Arc::clone(&stats),
        client.clone(),
    ));
    rt.spawn(crate::proxy::comms::run_loop(
        runtime_config,
        token_cache,
        client,
    ));

    Ok(ServedProxy {
        port: bound_port,
        stats,
    })
}

fn build_upstream_client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        // Why: IPv4-first — WSL2's localhost forwarder black-holes IPv6 SYNs, stalling `::1`
        // connects.
        .dns_resolver(Arc::new(crate::gateway::Ipv4FirstResolver))
        .pool_max_idle_per_host(16)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_mins(10))
        .build()
        .map_err(|e| std::io::Error::other(format!("upstream client build failed: {e}")))
}

async fn run_listener(listener: TcpListener, ctx: ProxyContext) {
    loop {
        let (stream, peer) = match listener.accept().await {
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
        _ = stream.set_nodelay(true);
        let conn_ctx = ctx.clone();
        tokio::spawn(async move {
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
}

pub async fn try_bind(port: u16) -> std::io::Result<TcpListener> {
    let v4: SocketAddr = SocketAddr::from(([127u8, 0, 0, 1], port));
    if let Ok(l) = TcpListener::bind(v4).await {
        return Ok(l);
    }
    let v6: SocketAddr = SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 1], port));
    TcpListener::bind(v6).await
}
