use std::collections::VecDeque;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use parking_lot::Mutex;

use serde::Deserialize;

use crate::gui::events::UiEvent;
use crate::gui::server_json::snapshot_to_json;
use crate::gui::server_util::{constant_time_eq, mint_csrf_token, now_unix, parse_query};
use crate::gui::state::AppState;
use crate::http_local::{ResponseBuilder, parse};
use crate::obs::output::diag;

const HTML: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/style.css");
const SCRIPT: &str = include_str!("../../web/app.js");
const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const LOG_CAPACITY: usize = 1000;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

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

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new()
    }
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
        let mut g = self.inner.lock();
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
        let g = self.inner.lock();
        g.entries.iter().filter(|e| e.id > since).cloned().collect()
    }
}

#[derive(Clone)]
pub struct Server {
    port: u16,
    csrf_token: String,
    log: ActivityLog,
}

impl Server {
    pub fn start(state: Arc<AppState>, tx: Sender<UiEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let csrf_token = mint_csrf_token();
        let log = ActivityLog::new();

        let csrf_clone = csrf_token.clone();
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
                let csrf_token = csrf_clone.clone();
                let log = log_clone.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, &state, &tx, &csrf_token, &log) {
                        diag(&format!("gui-server: connection: {e}"));
                    }
                });
            }
        });

        Ok(Server {
            port,
            csrf_token,
            log,
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port, self.csrf_token)
    }

    pub fn log(&self) -> &ActivityLog {
        &self.log
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: &Arc<AppState>,
    tx: &Sender<UiEvent>,
    csrf_token: &str,
    log: &ActivityLog,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;

    let req = match parse(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            return write_response(&mut stream, 400, "text/plain", e.as_bytes());
        },
    };

    let qs = parse_query(&req.query);
    let supplied = qs.get("t").map(String::as_str).unwrap_or("");
    if !constant_time_eq(supplied.as_bytes(), csrf_token.as_bytes()) {
        return write_response(
            &mut stream,
            403,
            "text/plain",
            b"forbidden: bad or missing ?t token",
        );
    }

    let route = (req.method.as_str(), req.path.as_str());
    match route {
        ("GET", "/") => serve_index(&mut stream, csrf_token),
        ("GET", "/api/state") => serve_state(&mut stream, state),
        ("GET", "/api/log") => {
            let since = qs
                .get("since")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            serve_log(&mut stream, log, since)
        },
        ("POST", path) => handle_action(&mut stream, tx, log, path, &req.body),
        _ => write_response(&mut stream, 404, "text/plain", b"not found"),
    }
}

fn serve_index(stream: &mut TcpStream, csrf_token: &str) -> std::io::Result<()> {
    let html = HTML
        .replace("__STYLE__", STYLE)
        .replace("__SCRIPT__", SCRIPT)
        .replace("__VERSION__", VERSION)
        .replace("__ICON_SVG__", ICON_SVG)
        .replace("__LOGO_SVG__", LOGO_SVG)
        .replace("__TOKEN__", csrf_token);
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

fn serve_state(stream: &mut TcpStream, state: &AppState) -> std::io::Result<()> {
    let snap = state.snapshot();
    match snapshot_to_json(&snap) {
        Ok(body) => write_response(stream, 200, "application/json", body.as_bytes()),
        Err(e) => write_response(
            stream,
            500,
            "text/plain",
            format!("encode error: {e}").as_bytes(),
        ),
    }
}

fn serve_log(stream: &mut TcpStream, log: &ActivityLog, since: u64) -> std::io::Result<()> {
    let entries = log.snapshot_since(since);
    match serde_json::to_string(&entries) {
        Ok(body) => write_response(stream, 200, "application/json", body.as_bytes()),
        Err(e) => write_response(
            stream,
            500,
            "text/plain",
            format!("encode error: {e}").as_bytes(),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    token: crate::auth::secret::Secret,
    #[serde(default)]
    gateway: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetGatewayBody {
    url: String,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
struct InstallProfileBody {
    path: String,
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
        "/api/gateway" => match serde_json::from_slice::<SetGatewayBody>(body) {
            Ok(b) => UiEvent::SetGatewayRequested(b.url),
            Err(e) => {
                return write_response(
                    stream,
                    400,
                    "text/plain",
                    format!("bad gateway body: {e}").as_bytes(),
                );
            },
        },
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "/api/claude/probe" => {
            UiEvent::Claude(crate::gui::claude::events::ClaudeUiEvent::ProbeRequested)
        },
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "/api/claude/profile/generate" => {
            UiEvent::Claude(crate::gui::claude::events::ClaudeUiEvent::ProfileGenerateRequested)
        },
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "/api/claude/profile/install" => match serde_json::from_slice::<InstallProfileBody>(body) {
            Ok(b) => UiEvent::Claude(
                crate::gui::claude::events::ClaudeUiEvent::ProfileInstallRequested(b.path),
            ),
            Err(e) => {
                return write_response(
                    stream,
                    400,
                    "text/plain",
                    format!("bad install body: {e}").as_bytes(),
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
    ResponseBuilder::new(status)
        .content_type(content_type)
        .body(body)
        .nosniff()
        .write(stream)
}
