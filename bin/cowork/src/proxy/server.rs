use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::http_local::{ResponseBuilder, parse};
use crate::output::diag;
use crate::proxy::{forward, secret};

pub use crate::http_local::Request;

const READ_TIMEOUT_HANDSHAKE: Duration = Duration::from_secs(30);

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

pub fn start(port: u16, gateway_base_url: String) -> std::io::Result<ProxyHandle> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let bound_port = listener.local_addr()?.port();
    let secret_value = secret::load_or_mint()?;
    let stats = Arc::new(ProxyStats::default());

    let stats_thread = stats.clone();
    let gateway = Arc::new(gateway_base_url);
    std::thread::Builder::new()
        .name("cowork-proxy-accept".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(e) => {
                        diag(&format!("proxy: accept failed: {e}"));
                        continue;
                    },
                };
                let stats = stats_thread.clone();
                let gateway = gateway.clone();
                let secret_value = secret_value.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &gateway, &secret_value, &stats) {
                        diag(&format!("proxy: connection: {e}"));
                    }
                });
            }
        })?;

    Ok(ProxyHandle {
        port: bound_port,
        stats,
    })
}

fn handle_connection(
    mut stream: TcpStream,
    gateway: &str,
    expected_secret: &str,
    stats: &ProxyStats,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT_HANDSHAKE))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;

    let req = match parse(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            return ResponseBuilder::new(400)
                .content_type("text/plain")
                .body(e.as_bytes())
                .write(&mut stream);
        },
    };

    let host = req.header("host").unwrap_or("");
    if !host_is_loopback(host) {
        return ResponseBuilder::new(403)
            .content_type("text/plain")
            .body(b"forbidden: non-loopback host\n")
            .write(&mut stream);
    }

    let auth = req.header("authorization").unwrap_or("");
    let presented = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .unwrap_or("");
    if presented.is_empty() || !secret::verify(presented, expected_secret) {
        return ResponseBuilder::new(403)
            .content_type("text/plain")
            .body(b"forbidden: bad loopback secret\n")
            .write(&mut stream);
    }

    if req.method == "GET" && req.path == "/healthz" {
        return ResponseBuilder::new(200)
            .content_type("text/plain")
            .body(b"ok\n")
            .write(&mut stream);
    }

    stream.set_read_timeout(None)?;

    let started = Instant::now();
    let status = forward::forward(&req, gateway, &mut stream).unwrap_or_else(|e| {
        diag(&format!("proxy: forward error: {e}"));
        0
    });
    let elapsed = started.elapsed().as_millis() as u64;
    stats.forwarded_total.fetch_add(1, Ordering::Relaxed);
    stats
        .last_forwarded_at_unix
        .store(now_unix(), Ordering::Relaxed);
    stats.last_status.store(status as u64, Ordering::Relaxed);
    stats.last_latency_ms.store(elapsed, Ordering::Relaxed);
    diag(&format!(
        "proxy: {} {} -> {} {}ms",
        req.method,
        req.target(),
        status,
        elapsed
    ));
    Ok(())
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
