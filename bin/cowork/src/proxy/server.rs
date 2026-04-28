use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::output::diag;
use crate::proxy::{forward, secret};

const READ_TIMEOUT_HANDSHAKE: Duration = Duration::from_secs(30);
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

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

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn handle_connection(
    mut stream: TcpStream,
    gateway: &str,
    expected_secret: &str,
    stats: &ProxyStats,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT_HANDSHAKE))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;

    let req = match parse_request(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            return write_simple(&mut stream, 400, "text/plain", e.as_bytes());
        },
    };

    let host = req.header("host").unwrap_or("");
    if !host_is_loopback(host) {
        return write_simple(
            &mut stream,
            403,
            "text/plain",
            b"forbidden: non-loopback host\n",
        );
    }

    let auth = req.header("authorization").unwrap_or("");
    let presented = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .unwrap_or("");
    if presented.is_empty() || !secret::verify(presented, expected_secret) {
        return write_simple(
            &mut stream,
            403,
            "text/plain",
            b"forbidden: bad loopback secret\n",
        );
    }

    if req.method == "GET" && req.path == "/healthz" {
        return write_simple(&mut stream, 200, "text/plain", b"ok\n");
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
        req.method, req.path, status, elapsed
    ));
    Ok(())
}

fn host_is_loopback(host: &str) -> bool {
    let host_only = host.split(':').next().unwrap_or("");
    matches!(host_only, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn parse_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("read request line: {e}"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "missing target".to_string())?
        .to_string();

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut total = request_line.len();
    let mut content_length = 0usize;
    let mut transfer_encoding: Option<String> = None;
    loop {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|e| format!("read header: {e}"))?;
        total += header.len();
        if total > MAX_HEADER_BYTES {
            return Err("headers too large".into());
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = match trimmed.split_once(':') {
            Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
            None => continue,
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().map_err(|e| e.to_string())?;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            transfer_encoding = Some(value.to_ascii_lowercase());
        }
        headers.push((name, value));
    }

    if content_length > MAX_BODY_BYTES {
        return Err(format!("body too large: {content_length}"));
    }

    let mut body = Vec::new();
    if let Some(te) = transfer_encoding.as_deref() {
        if te.contains("chunked") {
            read_chunked(&mut reader, &mut body)?;
        } else if content_length > 0 {
            body.resize(content_length, 0);
            reader
                .read_exact(&mut body)
                .map_err(|e| format!("read body: {e}"))?;
        }
    } else if content_length > 0 {
        body.resize(content_length, 0);
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {e}"))?;
    }

    Ok(Request {
        method,
        path: target,
        headers,
        body,
    })
}

fn read_chunked<R: BufRead>(reader: &mut R, out: &mut Vec<u8>) -> Result<(), String> {
    loop {
        let mut size_line = String::new();
        reader
            .read_line(&mut size_line)
            .map_err(|e| format!("read chunk size: {e}"))?;
        let size_str = size_line.trim_end_matches(['\r', '\n']);
        let size_str = size_str.split(';').next().unwrap_or("0").trim();
        let size =
            usize::from_str_radix(size_str, 16).map_err(|e| format!("chunk size parse: {e}"))?;
        if out.len() + size > MAX_BODY_BYTES {
            return Err("chunked body too large".into());
        }
        if size == 0 {
            let mut trailer = String::new();
            let _ = reader.read_line(&mut trailer);
            return Ok(());
        }
        let start = out.len();
        out.resize(start + size, 0);
        reader
            .read_exact(&mut out[start..])
            .map_err(|e| format!("read chunk: {e}"))?;
        let mut crlf = [0u8; 2];
        reader
            .read_exact(&mut crlf)
            .map_err(|e| format!("read chunk crlf: {e}"))?;
    }
}

fn write_simple(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: \
         {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
