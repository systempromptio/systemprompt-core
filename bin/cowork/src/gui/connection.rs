use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use serde::Deserialize;

use crate::gui::events::UiEvent;
use crate::gui::server::ActivityLog;
use crate::gui::server_json::snapshot_to_json;
use crate::gui::server_util::{constant_time_eq, parse_query};
use crate::gui::state::AppState;
use crate::http_local::{ResponseBuilder, parse};

const HTML: &str = include_str!("../../web/index.html");
const STYLE: &str = include_str!("../../web/style.css");
const SCRIPT: &str = include_str!("../../web/app.js");
const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const READ_TIMEOUT: Duration = Duration::from_secs(10);

const PLATFORM_SLUG: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

const PLATFORM_DISPLAY: &str = if cfg!(target_os = "macos") {
    "macOS"
} else if cfg!(target_os = "windows") {
    "Windows"
} else {
    "Linux"
};

pub struct ConnectionContext<'a> {
    pub state: &'a Arc<AppState>,
    pub tx: &'a Sender<UiEvent>,
    pub csrf_token: &'a str,
    pub log: &'a ActivityLog,
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

#[tracing::instrument(skip_all, fields(peer = ?stream.peer_addr().ok()))]
pub fn handle_connection(mut stream: TcpStream, ctx: ConnectionContext<'_>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;

    let req = match parse(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "request parse failed");
            return write_response(&mut stream, 400, "text/plain", e.as_bytes());
        },
    };

    let qs = parse_query(&req.query);
    let supplied = qs.get("t").map(String::as_str).unwrap_or("");
    if !constant_time_eq(supplied.as_bytes(), ctx.csrf_token.as_bytes()) {
        tracing::warn!(path = %req.path, "csrf token mismatch");
        return write_response(
            &mut stream,
            403,
            "text/plain",
            b"forbidden: bad or missing ?t token",
        );
    }

    tracing::debug!(method = %req.method, path = %req.path, "dispatching");
    let route = (req.method.as_str(), req.path.as_str());
    match route {
        ("GET", "/") => serve_index(&mut stream, ctx.csrf_token),
        ("GET", "/api/state") => serve_state(&mut stream, ctx.state),
        ("GET", "/api/marketplace") => serve_marketplace(&mut stream),
        ("GET", "/api/log") => {
            let since = qs
                .get("since")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            serve_log(&mut stream, ctx.log, since)
        },
        ("POST", path) => handle_action(&mut stream, ctx.tx, ctx.log, path, &req.body),
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
        .replace("__PLATFORM_DISPLAY__", PLATFORM_DISPLAY)
        .replace("__PLATFORM__", PLATFORM_SLUG)
        .replace("__TOKEN__", csrf_token);
    write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes())
}

fn serve_marketplace(stream: &mut TcpStream) -> std::io::Result<()> {
    let listing = crate::gui::server_marketplace::build_listing();
    match crate::gui::server_marketplace::listing_to_json(&listing) {
        Ok(body) => write_response(stream, 200, "application/json", body.as_bytes()),
        Err(e) => write_response(
            stream,
            500,
            "text/plain",
            format!("encode error: {e}").as_bytes(),
        ),
    }
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

#[tracing::instrument(skip(stream, tx, log, body), fields(action = path))]
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
        tracing::error!("event bridge closed");
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
