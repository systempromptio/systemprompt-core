use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::gui::events::UiEvent;
use crate::gui::state::{
    AppState, AppStateSnapshot, CachedToken, GatewayStatus, VerifiedIdentity,
};
use crate::output::diag;

const HTML: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/style.css");
const SCRIPT: &str = include_str!("../../web/app.js");
const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const LOG_CAPACITY: usize = 1000;
const READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct ActivityLog {
    inner: Arc<Mutex<LogState>>,
}

struct LogState {
    next_id: u64,
    entries: VecDeque<LogEntry>,
}

#[derive(Clone, serde::Serialize)]
struct LogEntry {
    id: u64,
    ts_unix: u64,
    line: String,
}

impl ActivityLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogState {
                next_id: 1,
                entries: VecDeque::with_capacity(LOG_CAPACITY),
            })),
        }
    }

    pub fn append(&self, line: impl Into<String>) {
        let mut g = self.inner.lock().expect("ActivityLog poisoned");
        let id = g.next_id;
        g.next_id += 1;
        let entry = LogEntry {
            id,
            ts_unix: now_unix(),
            line: line.into(),
        };
        if g.entries.len() == LOG_CAPACITY {
            g.entries.pop_front();
        }
        g.entries.push_back(entry);
    }

    fn snapshot_since(&self, since: u64) -> Vec<LogEntry> {
        let g = self.inner.lock().expect("ActivityLog poisoned");
        g.entries
            .iter()
            .filter(|e| e.id > since)
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
pub struct Server {
    port: u16,
    token: String,
    log: ActivityLog,
}

impl Server {
    pub fn start(state: Arc<AppState>, tx: Sender<UiEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let token = mint_token();
        let log = ActivityLog::new();

        let token_clone = token.clone();
        let log_clone = log.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(e) => {
                        diag(&format!("gui-server: accept failed: {e}"));
                        continue;
                    },
                };
                let state = state.clone();
                let tx = tx.clone();
                let token = token_clone.clone();
                let log = log_clone.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, state, tx, token, log) {
                        diag(&format!("gui-server: connection: {e}"));
                    }
                });
            }
        });

        Ok(Server { port, token, log })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port, self.token)
    }

    pub fn log(&self) -> &ActivityLog {
        &self.log
    }
}

fn mint_token() -> String {
    let mut hasher = Sha256::new();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    let stack_marker = 0u8;
    hasher.update((&stack_marker as *const u8 as usize).to_le_bytes());
    let mut buf = [0u8; 16];
    let digest = hasher.finalize();
    buf.copy_from_slice(&digest[..16]);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn handle_connection(
    mut stream: TcpStream,
    state: Arc<AppState>,
    tx: Sender<UiEvent>,
    token: String,
    log: ActivityLog,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;

    let req = match parse_request(&stream) {
        Ok(r) => r,
        Err(e) => {
            return write_response(&mut stream, 400, "text/plain", e.as_bytes());
        },
    };

    let qs = parse_query(&req.query);
    let supplied = qs.get("t").map(String::as_str).unwrap_or("");
    if !constant_time_eq(supplied.as_bytes(), token.as_bytes()) {
        return write_response(
            &mut stream,
            403,
            "text/plain",
            b"forbidden: bad or missing ?t token",
        );
    }

    let route = (req.method.as_str(), req.path.as_str());
    match route {
        ("GET", "/") => serve_index(&mut stream, &token),
        ("GET", "/api/state") => serve_state(&mut stream, &state),
        ("GET", "/api/log") => {
            let since = qs
                .get("since")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            serve_log(&mut stream, &log, since)
        },
        ("POST", path) => handle_action(&mut stream, &tx, &log, path, &req.body),
        _ => write_response(&mut stream, 404, "text/plain", b"not found"),
    }
}

fn parse_request(stream: &TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("read request line: {e}"))?;
    let mut parts = request_line.trim_end().split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let raw_target = parts
        .next()
        .ok_or_else(|| "missing target".to_string())?
        .to_string();

    let (path, query) = match raw_target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_target, String::new()),
    };

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|e| format!("read header: {e}"))?;
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix_ignore_ascii_case("content-length:") {
            content_length = rest
                .trim()
                .parse::<usize>()
                .map_err(|e| format!("content-length: {e}"))?;
        }
    }

    if content_length > MAX_BODY_BYTES {
        return Err(format!("body too large: {content_length}"));
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {e}"))?;
    }

    Ok(Request {
        method,
        path,
        query,
        body,
    })
}

trait StripPrefixIgnoreAsciiCase {
    fn strip_prefix_ignore_ascii_case<'a>(&'a self, prefix: &str) -> Option<&'a str>;
}

impl StripPrefixIgnoreAsciiCase for str {
    fn strip_prefix_ignore_ascii_case<'a>(&'a self, prefix: &str) -> Option<&'a str> {
        if self.len() < prefix.len() {
            return None;
        }
        let (head, tail) = self.split_at(prefix.len());
        if head.eq_ignore_ascii_case(prefix) {
            Some(tail)
        } else {
            None
        }
    }
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        out.insert(url_decode(k), url_decode(v));
    }
    out
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            },
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push(((hi << 4) | lo) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            },
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn serve_index(stream: &mut TcpStream, token: &str) -> std::io::Result<()> {
    let html = HTML
        .replace("__STYLE__", STYLE)
        .replace("__SCRIPT__", SCRIPT)
        .replace("__VERSION__", VERSION)
        .replace("__ICON_SVG__", ICON_SVG)
        .replace("__LOGO_SVG__", LOGO_SVG)
        .replace("__TOKEN__", token);
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

fn serve_state(stream: &mut TcpStream, state: &AppState) -> std::io::Result<()> {
    let snap = state.snapshot();
    let body = snapshot_to_json(&snap);
    write_response(stream, 200, "application/json", body.as_bytes())
}

fn serve_log(stream: &mut TcpStream, log: &ActivityLog, since: u64) -> std::io::Result<()> {
    let entries = log.snapshot_since(since);
    let body = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into());
    write_response(stream, 200, "application/json", body.as_bytes())
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    token: String,
    #[serde(default)]
    gateway: Option<String>,
}

fn handle_action(
    stream: &mut TcpStream,
    tx: &Sender<UiEvent>,
    log: &ActivityLog,
    path: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let event = match path {
        "/api/sync" => UiEvent::SyncRequested,
        "/api/validate" => UiEvent::ValidateRequested,
        "/api/probe" => UiEvent::GatewayProbeRequested,
        "/api/logout" => UiEvent::LogoutRequested,
        "/api/open_folder" => UiEvent::OpenConfigFolder,
        "/api/login" => match serde_json::from_slice::<LoginBody>(body) {
            Ok(b) => UiEvent::LoginRequested {
                token: b.token,
                gateway: b.gateway,
            },
            Err(e) => {
                return write_response(
                    stream,
                    400,
                    "text/plain",
                    format!("bad login body: {e}").as_bytes(),
                );
            },
        },
        _ => return write_response(stream, 404, "text/plain", b"not found"),
    };

    if tx.send(event).is_err() {
        log.append("ipc bridge closed");
        return write_response(stream, 500, "text/plain", b"event bridge closed");
    }
    write_response(stream, 204, "text/plain", b"")
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn snapshot_to_json(snap: &AppStateSnapshot) -> String {
    serde_json::json!({
        "gateway_url": snap.gateway_url,
        "config_file": snap.config_file,
        "pat_file": snap.pat_file,
        "config_present": snap.config_present,
        "pat_present": snap.pat_present,
        "plugins_dir": snap.plugins_dir,
        "last_sync_summary": snap.last_sync_summary,
        "skill_count": snap.skill_count,
        "agent_count": snap.agent_count,
        "plugin_count": snap.plugin_count,
        "sync_in_flight": snap.sync_in_flight,
        "last_action_message": snap.last_action_message,
        "cached_token": snap.cached_token.as_ref().map(cached_token_json),
        "gateway_status": gateway_status_json(&snap.gateway_status),
        "verified_identity": snap.verified_identity.as_ref().map(verified_identity_json),
        "signed_in": snap.signed_in(),
        "last_probe_at_unix": snap.last_probe_at_unix,
    })
    .to_string()
}

fn cached_token_json(t: &CachedToken) -> serde_json::Value {
    serde_json::json!({
        "ttl_seconds": t.ttl_seconds,
        "length": t.length,
    })
}

fn gateway_status_json(s: &GatewayStatus) -> serde_json::Value {
    match s {
        GatewayStatus::Unknown => serde_json::json!({"state": "unknown"}),
        GatewayStatus::Probing => serde_json::json!({"state": "probing"}),
        GatewayStatus::Reachable { latency_ms } => {
            serde_json::json!({"state": "reachable", "latency_ms": latency_ms})
        },
        GatewayStatus::Unreachable { reason } => {
            serde_json::json!({"state": "unreachable", "reason": reason})
        },
    }
}

fn verified_identity_json(v: &VerifiedIdentity) -> serde_json::Value {
    serde_json::json!({
        "email": v.email,
        "user_id": v.user_id,
        "tenant_id": v.tenant_id,
        "exp_unix": v.exp_unix,
        "verified_at_unix": v.verified_at_unix,
    })
}
