use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::gui::action_dispatch::handle_action;
use crate::gui::events::UiEvent;
use crate::gui::server::ActivityLog;
use crate::gui::server_json::snapshot_to_json;
use crate::gui::server_util::{constant_time_eq, parse_query};
use crate::gui::state::AppState;
use crate::http_local::{ResponseBuilder, parse_from_read};

const HTML: &str = include_str!("../../web/index.html");

const CSS_FILES: &[(&str, &str)] = &[
    ("tokens", include_str!("../../web/css/tokens.css")),
    ("fonts", include_str!("../../web/css/fonts.css")),
    ("reset", include_str!("../../web/css/reset.css")),
    ("kbd", include_str!("../../web/css/kbd.css")),
    ("dot", include_str!("../../web/css/dot.css")),
    ("badge", include_str!("../../web/css/badge.css")),
    ("button", include_str!("../../web/css/button.css")),
    ("topbar", include_str!("../../web/css/topbar.css")),
    ("rail", include_str!("../../web/css/rail.css")),
    ("shell", include_str!("../../web/css/shell.css")),
    ("drawer", include_str!("../../web/css/drawer.css")),
    (
        "marketplace-base",
        include_str!("../../web/css/marketplace-base.css"),
    ),
    (
        "marketplace-list",
        include_str!("../../web/css/marketplace-list.css"),
    ),
    (
        "marketplace-detail",
        include_str!("../../web/css/marketplace-detail.css"),
    ),
    ("status", include_str!("../../web/css/status.css")),
    ("settings", include_str!("../../web/css/settings.css")),
    ("setup", include_str!("../../web/css/setup.css")),
    ("agents", include_str!("../../web/css/agents.css")),
    ("log", include_str!("../../web/css/log.css")),
    ("footer", include_str!("../../web/css/footer.css")),
    ("responsive", include_str!("../../web/css/responsive.css")),
    ("main", include_str!("../../web/css/main.css")),
];

const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.woff2");
const FONT_INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.woff2");
const FONT_OPENSANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.woff2");
const FONT_OPENSANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Bold.woff2");

const JS_MODULES: &[(&str, &str)] = &[
    ("agents", include_str!("../../web/js/agents.js")),
    ("api", include_str!("../../web/js/api.js")),
    ("cloud", include_str!("../../web/js/cloud.js")),
    ("crumb", include_str!("../../web/js/crumb.js")),
    ("dom", include_str!("../../web/js/dom.js")),
    ("drawer", include_str!("../../web/js/drawer.js")),
    ("footer", include_str!("../../web/js/footer.js")),
    ("hosts", include_str!("../../web/js/hosts.js")),
    ("index", include_str!("../../web/js/index.js")),
    ("marketplace", include_str!("../../web/js/marketplace.js")),
    (
        "overall-badge",
        include_str!("../../web/js/overall-badge.js"),
    ),
    ("profile", include_str!("../../web/js/profile.js")),
    ("proxy", include_str!("../../web/js/proxy.js")),
    (
        "rail-indicator",
        include_str!("../../web/js/rail-indicator.js"),
    ),
    ("setup", include_str!("../../web/js/setup.js")),
    ("state", include_str!("../../web/js/state.js")),
    ("sync-pill", include_str!("../../web/js/sync-pill.js")),
    ("tabs", include_str!("../../web/js/tabs.js")),
    (
        "events/keyboard",
        include_str!("../../web/js/events/keyboard.js"),
    ),
    (
        "events/registry",
        include_str!("../../web/js/events/registry.js"),
    ),
    (
        "marketplace/detail",
        include_str!("../../web/js/marketplace/detail.js"),
    ),
    (
        "marketplace/glyph",
        include_str!("../../web/js/marketplace/glyph.js"),
    ),
    (
        "marketplace/list",
        include_str!("../../web/js/marketplace/list.js"),
    ),
    (
        "marketplace/state",
        include_str!("../../web/js/marketplace/state.js"),
    ),
    (
        "setup/gateway",
        include_str!("../../web/js/setup/gateway.js"),
    ),
    (
        "setup/agents",
        include_str!("../../web/js/setup/agents.js"),
    ),
    ("setup/mode", include_str!("../../web/js/setup/mode.js")),
    ("hosts/card", include_str!("../../web/js/hosts/card.js")),
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
    let is_static_asset = req.method == "GET"
        && (req.path.starts_with("/assets/css/")
            || req.path.starts_with("/assets/js/")
            || req.path.starts_with("/assets/fonts/"));
    if !is_static_asset {
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
    }

    tracing::debug!(method = %req.method, path = %req.path, "dispatching");
    let route = (req.method.as_str(), req.path.as_str());
    if let ("GET", path) = route {
        if let Some(asset) = serve_static_asset(path, ctx.csrf_token) {
            return write_response(&mut stream, 200, asset.content_type, &asset.body);
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

struct StaticAsset {
    content_type: &'static str,
    body: Vec<u8>,
}

fn serve_static_asset(path: &str, csrf_token: &str) -> Option<StaticAsset> {
    if let Some(name) = path
        .strip_prefix("/assets/css/")
        .and_then(|s| s.strip_suffix(".css"))
    {
        if let Some((_, src)) = CSS_FILES.iter().find(|(n, _)| *n == name) {
            return Some(StaticAsset {
                content_type: "text/css; charset=utf-8",
                body: src.replace("__TOKEN__", csrf_token).into_bytes(),
            });
        }
    }
    if let Some(name) = path
        .strip_prefix("/assets/js/")
        .and_then(|s| s.strip_suffix(".js"))
    {
        if let Some((_, src)) = JS_MODULES.iter().find(|(n, _)| *n == name) {
            return Some(StaticAsset {
                content_type: "application/javascript; charset=utf-8",
                body: src.replace("__TOKEN__", csrf_token).into_bytes(),
            });
        }
    }
    None
}

fn serve_index(stream: &mut TcpStream, csrf_token: &str) -> std::io::Result<()> {
    let html = HTML
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
