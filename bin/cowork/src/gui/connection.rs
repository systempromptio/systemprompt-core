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
use crate::http_local::{ResponseBuilder, parse_from_read};

const HTML: &str = include_str!("../../web/index.html");
const STYLE_PARTS: &[&str] = &[
    include_str!("../../web/css/tokens.css"),
    include_str!("../../web/css/fonts.css"),
    include_str!("../../web/css/reset.css"),
    include_str!("../../web/css/kbd.css"),
    include_str!("../../web/css/dot.css"),
    include_str!("../../web/css/badge.css"),
    include_str!("../../web/css/button.css"),
    include_str!("../../web/css/topbar.css"),
    include_str!("../../web/css/rail.css"),
    include_str!("../../web/css/shell.css"),
    include_str!("../../web/css/drawer.css"),
    include_str!("../../web/css/marketplace-base.css"),
    include_str!("../../web/css/marketplace-list.css"),
    include_str!("../../web/css/marketplace-detail.css"),
    include_str!("../../web/css/status.css"),
    include_str!("../../web/css/settings.css"),
    include_str!("../../web/css/setup.css"),
    include_str!("../../web/css/agents.css"),
    include_str!("../../web/css/log.css"),
    include_str!("../../web/css/footer.css"),
    include_str!("../../web/css/responsive.css"),
];

fn style_concat() -> String {
    STYLE_PARTS.join("\n")
}
const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.woff2");
const FONT_INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.woff2");
const FONT_OPENSANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.woff2");
const FONT_OPENSANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Bold.woff2");

const JS_MODULES: &[(&str, &str)] = &[
    ("main", include_str!("../../web/js/main.js")),
    ("api", include_str!("../../web/js/api.js")),
    ("dom", include_str!("../../web/js/dom.js")),
    ("activity", include_str!("../../web/js/activity.js")),
    ("tabs", include_str!("../../web/js/tabs.js")),
    ("setup", include_str!("../../web/js/setup.js")),
    ("snapshot", include_str!("../../web/js/snapshot.js")),
    ("marketplace", include_str!("../../web/js/marketplace.js")),
];

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


#[derive(Debug, Deserialize)]
struct InstallProfileBody {
    path: String,
}


fn parse_host_id(path: &str, prefix: &str, suffix: &str) -> Option<String> {
    let rest = path.strip_prefix(prefix)?;
    let id = rest.strip_suffix(suffix)?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id.to_string())
}

#[tracing::instrument(skip_all, fields(peer = ?stream.peer_addr().ok()))]
pub fn handle_connection(
    mut stream: TcpStream,
    ctx: &ConnectionContext<'_>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;

    let req = match parse_from_read(&mut stream) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "request parse failed");
            return write_response(&mut stream, 400, "text/plain", e.to_string().as_bytes());
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
    if let ("GET", path) = route {
        if let Some(name) = path
            .strip_prefix("/assets/js/")
            .and_then(|s| s.strip_suffix(".js"))
        {
            if let Some((_, src)) = JS_MODULES.iter().find(|(n, _)| *n == name) {
                let body = src.replace("__TOKEN__", ctx.csrf_token);
                return write_response(
                    &mut stream,
                    200,
                    "application/javascript; charset=utf-8",
                    body.as_bytes(),
                );
            }
        }
    }
    match route {
        ("GET", "/") => serve_index(&mut stream, ctx.csrf_token),
        ("GET", "/assets/fonts/Inter-Regular.woff2") => {
            write_response(&mut stream, 200, "font/woff2", FONT_INTER_REGULAR)
        },
        ("GET", "/assets/fonts/Inter-Bold.woff2") => {
            write_response(&mut stream, 200, "font/woff2", FONT_INTER_BOLD)
        },
        ("GET", "/assets/fonts/OpenSans-Regular.woff2") => {
            write_response(&mut stream, 200, "font/woff2", FONT_OPENSANS_REGULAR)
        },
        ("GET", "/assets/fonts/OpenSans-Bold.woff2") => {
            write_response(&mut stream, 200, "font/woff2", FONT_OPENSANS_BOLD)
        },
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
        .replace("__STYLE__", &style_concat())
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
        
        "/api/proxy/probe" => {
            UiEvent::Host(crate::gui::hosts::events::HostUiEvent::ProxyProbeRequested)
        },
        
        p if p.starts_with("/api/hosts/") && p.ends_with("/probe") => {
            match parse_host_id(p, "/api/hosts/", "/probe") {
                Some(host_id) => {
                    UiEvent::Host(crate::gui::hosts::events::HostUiEvent::ProbeRequested {
                        host_id,
                    })
                },
                None => return write_response(stream, 404, "text/plain", b"not found"),
            }
        },
        
        p if p.starts_with("/api/hosts/") && p.ends_with("/profile/generate") => {
            match parse_host_id(p, "/api/hosts/", "/profile/generate") {
                Some(host_id) => UiEvent::Host(
                    crate::gui::hosts::events::HostUiEvent::ProfileGenerateRequested { host_id },
                ),
                None => return write_response(stream, 404, "text/plain", b"not found"),
            }
        },
        
        p if p.starts_with("/api/hosts/") && p.ends_with("/profile/install") => {
            match parse_host_id(p, "/api/hosts/", "/profile/install") {
                Some(host_id) => match serde_json::from_slice::<InstallProfileBody>(body) {
                    Ok(b) => UiEvent::Host(
                        crate::gui::hosts::events::HostUiEvent::ProfileInstallRequested {
                            host_id,
                            path: b.path,
                        },
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
                None => return write_response(stream, 404, "text/plain", b"not found"),
            }
        },

        p if p.starts_with("/api/agents/") && p.ends_with("/uninstall") => {
            match parse_host_id(p, "/api/agents/", "/uninstall") {
                Some(host_id) => UiEvent::AgentUninstall { host_id },
                None => return write_response(stream, 404, "text/plain", b"not found"),
            }
        },

        p if p.starts_with("/api/agents/") && p.ends_with("/open-config") => {
            match parse_host_id(p, "/api/agents/", "/open-config") {
                Some(host_id) => UiEvent::AgentOpenConfig { host_id },
                None => return write_response(stream, 404, "text/plain", b"not found"),
            }
        },

        "/api/setup/complete" => UiEvent::SetupComplete,

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
