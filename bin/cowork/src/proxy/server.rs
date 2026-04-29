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

use systemprompt_identifiers::ValidatedUrl;

use crate::ids::ProxySecret;
use crate::obs::output::diag;
use crate::proxy::{forward, secret};

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
}

#[derive(Clone)]
pub(super) struct ProxyContext {
    pub gateway_base: Arc<ValidatedUrl>,
    pub secret: Arc<ProxySecret>,
    pub stats: Arc<ProxyStats>,
    pub client: reqwest::Client,
}

pub fn start(rt: &Runtime, port: u16, gateway_base_url: &ValidatedUrl) -> std::io::Result<ProxyHandle> {
    let gateway_base = gateway_base_url.clone();
    let loopback = secret::load_or_mint_typed()?;
    let proxy_secret = ProxySecret::new(loopback.into_inner());
    let stats = Arc::new(ProxyStats::default());

    let client = build_upstream_client()?;

    let ctx = ProxyContext {
        gateway_base: Arc::new(gateway_base),
        secret: Arc::new(proxy_secret),
        stats: stats.clone(),
        client,
    };

    let (port_tx, port_rx) = oneshot::channel::<std::io::Result<u16>>();
    rt.spawn(run_listener(port, ctx, port_tx));

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
        let (stream, _peer) = match listener.accept().await {
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
            let svc = service_fn(move |req| handle_request(req, conn_ctx.clone()));
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
    let v6: SocketAddr = SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 0], port));
    match TcpListener::bind(v6).await {
        Ok(l) => Ok(l),
        Err(_) => {
            let v4: SocketAddr = SocketAddr::from(([127u8, 0, 0, 1], port));
            TcpListener::bind(v4).await
        },
    }
}

async fn handle_request(
    req: Request<Incoming>,
    ctx: ProxyContext,
) -> Result<Response<forward::ProxyBody>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    if let Some(host) = req
        .headers()
        .get(http::header::HOST)
        .and_then(|v| v.to_str().ok())
    {
        if !host_is_loopback(host) {
            return Ok(simple_response(
                StatusCode::FORBIDDEN,
                "forbidden: non-loopback host\n",
            ));
        }
    }

    let presented = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.trim_start_matches("Bearer ")
                .trim_start_matches("bearer ")
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    if presented.is_empty() || !secret::verify(&presented, ctx.secret.as_ref()) {
        return Ok(simple_response(
            StatusCode::FORBIDDEN,
            "forbidden: bad loopback secret\n",
        ));
    }

    if method == Method::GET && path == "/healthz" {
        return Ok(simple_response(StatusCode::OK, "ok\n"));
    }

    let started = Instant::now();
    match forward::forward(req, ctx.client.clone(), ctx.gateway_base.as_ref()).await {
        Ok(response) => {
            let status = response.status().as_u16();
            record_stats(&ctx.stats, status, started);
            diag(&format!(
                "proxy: {method} {path} -> {status} {}ms",
                started.elapsed().as_millis()
            ));
            Ok(response)
        },
        Err(e) => {
            record_stats(&ctx.stats, StatusCode::BAD_GATEWAY.as_u16(), started);
            if forward::is_client_disconnect(&e) {
                diag(&format!("proxy: {method} {path} -> client disconnected"));
            } else {
                diag(&format!("proxy: {method} {path} -> forward error: {e}"));
            }
            Ok(simple_response(StatusCode::BAD_GATEWAY, "bad gateway\n"))
        },
    }
}

fn record_stats(stats: &ProxyStats, status: u16, started: Instant) {
    stats.forwarded_total.fetch_add(1, Ordering::Relaxed);
    stats
        .last_forwarded_at_unix
        .store(now_unix(), Ordering::Relaxed);
    stats
        .last_status
        .store(u64::from(status), Ordering::Relaxed);
    stats
        .last_latency_ms
        .store(
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
}

fn simple_response(status: StatusCode, body: &'static str) -> Response<forward::ProxyBody> {
    let full = Full::new(Bytes::from_static(body.as_bytes()))
        .map_err(|never| match never {})
        .boxed();
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain")
        .header(http::header::CONNECTION, "close")
        .body(full)
        .unwrap_or_else(|_| {
            Response::new(
                Full::new(Bytes::new())
                    .map_err(|never| match never {})
                    .boxed(),
            )
        })
}

fn host_is_loopback(host: &str) -> bool {
    let host_only = host.split(':').next().unwrap_or("");
    matches!(host_only, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
