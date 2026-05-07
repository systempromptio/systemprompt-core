use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

use crate::config::{RuntimeConfig, SharedRuntimeConfig};
use crate::ids::ProxySecret;
use crate::obs::output::diag;
use crate::proxy::session::SessionContext;
use crate::proxy::token_cache::TokenCache;
use crate::proxy::{forward, heartbeat, secret};

#[derive(Clone)]
pub struct ProxyHandle {
    pub port: u16,
    pub stats: Arc<ProxyStats>,
}

#[derive(Default)]
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
pub(super) struct ProxyContext {
    pub runtime_config: SharedRuntimeConfig,
    pub secret: Arc<ProxySecret>,
    pub stats: Arc<ProxyStats>,
    pub client: reqwest::Client,
    pub token_cache: Arc<TokenCache>,
    pub session: Arc<SessionContext>,
}

impl ProxyContext {
    pub fn snapshot(&self) -> Arc<RuntimeConfig> {
        self.runtime_config.load_full()
    }
}

pub fn start(
    rt: &Runtime,
    port: u16,
    runtime_config: SharedRuntimeConfig,
    token_cache: Arc<TokenCache>,
    session: Arc<SessionContext>,
) -> std::io::Result<ProxyHandle> {
    let loopback = secret::proxy_init()?;
    let proxy_secret = ProxySecret::new(loopback.into_inner());
    let stats = Arc::new(ProxyStats::default());

    let client = build_upstream_client()?;

    let ctx = ProxyContext {
        runtime_config: runtime_config.clone(),
        secret: Arc::new(proxy_secret),
        stats: stats.clone(),
        client: client.clone(),
        token_cache: token_cache.clone(),
        session: session.clone(),
    };

    let (port_tx, port_rx) = oneshot::channel::<std::io::Result<u16>>();
    rt.spawn(run_listener(port, ctx, port_tx));
    rt.spawn(heartbeat::run_loop(
        runtime_config,
        token_cache,
        session,
        Arc::clone(&stats),
        client,
    ));

    let bound_port = match port_rx.blocking_recv() {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(std::io::Error::other("proxy listener task aborted")),
    };

    Ok(ProxyHandle {
        port: bound_port,
        stats,
    })
}

fn build_upstream_client() -> std::io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .pool_max_idle_per_host(16)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| std::io::Error::other(format!("upstream client build failed: {e}")))
}

async fn run_listener(
    port: u16,
    ctx: ProxyContext,
    port_tx: oneshot::Sender<std::io::Result<u16>>,
) {
    let listener = match bind_listener(port).await {
        Ok(l) => l,
        Err(e) => {
            let _ = port_tx.send(Err(e));
            return;
        },
    };
    let bound = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(e) => {
            let _ = port_tx.send(Err(e));
            return;
        },
    };
    let _ = port_tx.send(Ok(bound));

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(t) => t,
            Err(e) => {
                diag(&format!("proxy: accept failed: {e}"));
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            },
        };
        let _ = stream.set_nodelay(true);
        let conn_ctx = ctx.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| handle_request(req, conn_ctx.clone(), peer));
            if let Err(e) = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(io, svc)
                .await
            {
                let msg = e.to_string();
                if !msg.contains("closed") && !msg.contains("connection") {
                    diag(&format!("proxy: connection: {msg}"));
                }
            }
        });
    }
}

async fn bind_listener(port: u16) -> std::io::Result<TcpListener> {
    let v4: SocketAddr = SocketAddr::from(([127u8, 0, 0, 1], port));
    if let Ok(l) = TcpListener::bind(v4).await {
        return Ok(l);
    }
    let v6: SocketAddr = SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 1], port));
    TcpListener::bind(v6).await
}

async fn handle_request(
    req: Request<Incoming>,
    ctx: ProxyContext,
    peer: SocketAddr,
) -> Result<Response<forward::ProxyBody>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let req_id = mint_req_id();
    let host_hdr = req
        .headers()
        .get(http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let user_agent = req
        .headers()
        .get(http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_length = req
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    tracing::info!(
        target: "systemprompt_bridge::proxy",
        req_id = %req_id,
        method = %method,
        path = %path,
        query = %query,
        peer = %peer,
        host = %host_hdr,
        ua = %user_agent,
        content_length,
        "req in"
    );

    if !host_hdr.is_empty() && !host_is_loopback(&host_hdr) {
        tracing::warn!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            host = %host_hdr,
            peer = %peer,
            "reject: non-loopback host"
        );
        crate::activity::activity_log().append(format!(
            "proxy: {method} {path} → 403 (non-loopback host: {host_hdr}) [{req_id}]"
        ));
        return Ok(simple_response(
            StatusCode::FORBIDDEN,
            "forbidden: non-loopback host\n",
        ));
    }

    if is_unauthenticated_path(&method, &path) {
        tracing::debug!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            method = %method,
            path = %path,
            "unauthenticated path"
        );
        if path == "/healthz" {
            return Ok(health_response(&method));
        }
        return forward_to_gateway(req, ctx, req_id, method, path).await;
    }

    let presented = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .unwrap_or(v)
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    if presented.is_empty() || !secret::verify(&presented, ctx.secret.as_ref()) {
        let presented_fp = sha256_8(&presented);
        let expected_fp = sha256_8(ctx.secret.as_ref().as_str());
        tracing::warn!(
            target: "systemprompt_bridge::proxy",
            req_id = %req_id,
            peer = %peer,
            method = %method,
            path = %path,
            ua = %user_agent,
            presented_len = presented.len(),
            presented_fp = %presented_fp,
            expected_fp = %expected_fp,
            "reject: bad loopback secret"
        );
        crate::activity::activity_log().append(format!(
            "proxy: {method} {path} → 403 (bad secret; presented_fp={presented_fp} \
             expected_fp={expected_fp}) [{req_id}]"
        ));
        return Ok(simple_response(
            StatusCode::FORBIDDEN,
            "forbidden: bad loopback secret\n",
        ));
    }

    forward_to_gateway(req, ctx, req_id, method, path).await
}

async fn forward_to_gateway(
    req: Request<Incoming>,
    ctx: ProxyContext,
    req_id: String,
    method: Method,
    path: String,
) -> Result<Response<forward::ProxyBody>, Infallible> {
    let started = Instant::now();
    let cfg = ctx.snapshot();
    match forward::forward(
        req,
        ctx.client.clone(),
        cfg.gateway_base.as_ref(),
        ctx.token_cache.as_ref(),
        ctx.session.as_ref(),
        Arc::clone(&ctx.stats),
    )
    .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, status, latency_ms);
            tracing::info!(
                target: "systemprompt_bridge::proxy",
                req_id = %req_id,
                method = %method,
                path = %path,
                status,
                latency_ms,
                "req out"
            );
            crate::activity::activity_log().append(format!(
                "proxy: {method} {path} → {status} ({latency_ms}ms) [{req_id}]"
            ));
            Ok(response)
        },
        Err(e) => {
            let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            record_stats(&ctx.stats, StatusCode::BAD_GATEWAY.as_u16(), latency_ms);
            if forward::is_client_disconnect(&e) {
                tracing::warn!(
                    target: "systemprompt_bridge::proxy",
                    req_id = %req_id,
                    method = %method,
                    path = %path,
                    latency_ms,
                    "req out: client disconnected"
                );
                crate::activity::activity_log().append(format!(
                    "proxy: {method} {path} → client disconnected [{req_id}]"
                ));
            } else {
                tracing::error!(
                    target: "systemprompt_bridge::proxy",
                    req_id = %req_id,
                    method = %method,
                    path = %path,
                    latency_ms,
                    error = %e,
                    "req out: forward error"
                );
                crate::activity::activity_log()
                    .append(format!("proxy: {method} {path} → error: {e} [{req_id}]"));
            }
            Ok(simple_response(StatusCode::BAD_GATEWAY, "bad gateway\n"))
        },
    }
}

fn is_unauthenticated_path(method: &Method, path: &str) -> bool {
    match (method, path) {
        (&Method::GET | &Method::HEAD, "/healthz") => true,
        (&Method::POST, p) if p == "/otel" || p.starts_with("/otel/") => true,
        _ => false,
    }
}

fn health_response(method: &Method) -> Response<forward::ProxyBody> {
    let body = if method == Method::HEAD { "" } else { "ok\n" };
    simple_response(StatusCode::OK, body)
}

fn mint_req_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

fn sha256_8(s: &str) -> String {
    if s.is_empty() {
        return "<empty>".to_string();
    }
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(s.as_bytes());
    format!("{:08x}", u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
}

fn record_stats(stats: &ProxyStats, status: u16, latency_ms: u64) {
    stats.forwarded_total.fetch_add(1, Ordering::Relaxed);
    stats
        .last_forwarded_at_unix
        .store(now_unix(), Ordering::Relaxed);
    stats
        .last_status
        .store(u64::from(status), Ordering::Relaxed);
    stats.last_latency_ms.store(latency_ms, Ordering::Relaxed);
}

fn simple_response(status: StatusCode, body: &'static str) -> Response<forward::ProxyBody> {
    let full = Full::new(Bytes::from_static(body.as_bytes()))
        .map_err(|never| match never {})
        .boxed();
    let mut resp = Response::new(full);
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("text/plain"),
    );
    resp.headers_mut().insert(
        http::header::CONNECTION,
        http::HeaderValue::from_static("close"),
    );
    resp
}

fn host_is_loopback(host: &str) -> bool {
    let host_only = host.split(':').next().unwrap_or("");
    matches!(host_only, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
