use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::gui::action_dispatch::handle_action;
use crate::gui::assets;
use crate::gui::events::UiEvent;
use crate::gui::server::ActivityLog;
use crate::gui::server_json::snapshot_to_json;
use crate::gui::server_util::{constant_time_eq, parse_query};
use crate::gui::state::AppState;
use crate::http_local::{ResponseBuilder, parse_from_read};

const READ_TIMEOUT: Duration = Duration::from_secs(10);

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
    if let ("GET", path) = (req.method.as_str(), req.path.as_str()) {
        if let Some(asset) = assets::lookup_path(path, ctx.csrf_token) {
            return write_response(&mut stream, 200, asset.content_type, &asset.body);
        }
    }
    match (req.method.as_str(), req.path.as_str()) {
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
