use std::net::TcpStream;
use std::sync::mpsc::Sender;

use serde::Deserialize;

use crate::gui::events::UiEvent;
use crate::gui::server::ActivityLog;
use crate::http_local::ResponseBuilder;

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

pub(super) enum ActionOutcome {
    Event(UiEvent),
    Response {
        status: u16,
        content_type: &'static str,
        body: Vec<u8>,
    },
}

#[tracing::instrument(skip(stream, tx, log, body), fields(action = path))]
pub(super) fn handle_action(
    stream: &mut TcpStream,
    tx: &Sender<UiEvent>,
    log: &ActivityLog,
    path: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let outcome = match parse_action(path, body) {
        Some(o) => o,
        None => {
            return write_response(stream, 404, "text/plain", b"not found");
        },
    };
    match outcome {
        ActionOutcome::Event(event) => dispatch_event(stream, tx, log, event),
        ActionOutcome::Response {
            status,
            content_type,
            body,
        } => write_response(stream, status, content_type, &body),
    }
}

fn parse_action(path: &str, body: &[u8]) -> Option<ActionOutcome> {
    if let Some(o) = parse_simple(path) {
        return Some(o);
    }
    if let Some(o) = parse_with_body(path, body) {
        return Some(o);
    }
    parse_dynamic(path, body)
}

fn parse_simple(path: &str) -> Option<ActionOutcome> {
    let event = match path {
        "/api/sync" => UiEvent::SyncRequested,
        "/api/validate" => UiEvent::ValidateRequested,
        "/api/probe" => UiEvent::GatewayProbeRequested,
        "/api/logout" => UiEvent::LogoutRequested,
        "/api/open_folder" => UiEvent::OpenConfigFolder,
        "/api/setup/complete" => UiEvent::SetupComplete,
        "/api/proxy/probe" => {
            UiEvent::Host(crate::gui::hosts::events::HostUiEvent::ProxyProbeRequested)
        },
        _ => return None,
    };
    Some(ActionOutcome::Event(event))
}

fn parse_with_body(path: &str, body: &[u8]) -> Option<ActionOutcome> {
    match path {
        "/api/gateway" => Some(parse_set_gateway(body)),
        "/api/login" => Some(parse_login(body)),
        _ => None,
    }
}

fn parse_set_gateway(body: &[u8]) -> ActionOutcome {
    match serde_json::from_slice::<SetGatewayBody>(body) {
        Ok(b) => ActionOutcome::Event(UiEvent::SetGatewayRequested(b.url)),
        Err(e) => bad_request(format!("bad gateway body: {e}")),
    }
}

fn parse_login(body: &[u8]) -> ActionOutcome {
    match serde_json::from_slice::<LoginBody>(body) {
        Ok(b) => ActionOutcome::Event(UiEvent::LoginRequested {
            token: b.token,
            gateway: b.gateway,
        }),
        Err(e) => bad_request(format!("bad login body: {e}")),
    }
}

fn parse_dynamic(path: &str, body: &[u8]) -> Option<ActionOutcome> {
    if path.starts_with("/api/hosts/") {
        return parse_hosts(path, body);
    }
    if path.starts_with("/api/agents/") {
        return parse_agents(path);
    }
    None
}

fn parse_hosts(path: &str, body: &[u8]) -> Option<ActionOutcome> {
    if path.ends_with("/probe") {
        return Some(host_id_event(path, "/api/hosts/", "/probe", |host_id| {
            UiEvent::Host(crate::gui::hosts::events::HostUiEvent::ProbeRequested { host_id })
        }));
    }
    if path.ends_with("/profile/generate") {
        return Some(host_id_event(
            path,
            "/api/hosts/",
            "/profile/generate",
            |host_id| {
                UiEvent::Host(
                    crate::gui::hosts::events::HostUiEvent::ProfileGenerateRequested { host_id },
                )
            },
        ));
    }
    if path.ends_with("/profile/install") {
        return Some(parse_profile_install(path, body));
    }
    None
}

fn parse_profile_install(path: &str, body: &[u8]) -> ActionOutcome {
    let host_id = match parse_host_id(path, "/api/hosts/", "/profile/install") {
        Some(id) => id,
        None => return not_found(),
    };
    match serde_json::from_slice::<InstallProfileBody>(body) {
        Ok(b) => ActionOutcome::Event(UiEvent::Host(
            crate::gui::hosts::events::HostUiEvent::ProfileInstallRequested {
                host_id,
                path: b.path,
            },
        )),
        Err(e) => bad_request(format!("bad install body: {e}")),
    }
}

fn parse_agents(path: &str) -> Option<ActionOutcome> {
    if path.ends_with("/uninstall") {
        return Some(host_id_event(
            path,
            "/api/agents/",
            "/uninstall",
            |host_id| UiEvent::AgentUninstall { host_id },
        ));
    }
    if path.ends_with("/open-config") {
        return Some(host_id_event(
            path,
            "/api/agents/",
            "/open-config",
            |host_id| UiEvent::AgentOpenConfig { host_id },
        ));
    }
    None
}

fn host_id_event(
    path: &str,
    prefix: &str,
    suffix: &str,
    build: impl FnOnce(String) -> UiEvent,
) -> ActionOutcome {
    match parse_host_id(path, prefix, suffix) {
        Some(host_id) => ActionOutcome::Event(build(host_id)),
        None => not_found(),
    }
}

fn parse_host_id(path: &str, prefix: &str, suffix: &str) -> Option<String> {
    let rest = path.strip_prefix(prefix)?;
    let id = rest.strip_suffix(suffix)?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id.to_string())
}

fn dispatch_event(
    stream: &mut TcpStream,
    tx: &Sender<UiEvent>,
    log: &ActivityLog,
    event: UiEvent,
) -> std::io::Result<()> {
    if tx.send(event).is_err() {
        log.append("ipc bridge closed");
        tracing::error!("event bridge closed");
        return write_response(stream, 500, "text/plain", b"event bridge closed");
    }
    write_response(stream, 204, "text/plain", b"")
}

fn bad_request(msg: String) -> ActionOutcome {
    ActionOutcome::Response {
        status: 400,
        content_type: "text/plain",
        body: msg.into_bytes(),
    }
}

fn not_found() -> ActionOutcome {
    ActionOutcome::Response {
        status: 404,
        content_type: "text/plain",
        body: b"not found".to_vec(),
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
