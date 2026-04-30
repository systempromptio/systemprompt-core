use serde_json::{Value, json};

use crate::gui::GuiApp;
use crate::gui::command::{self, CommandOutcome};
use crate::gui::events::UiEvent;
use crate::gui::ipc::{self, BridgeError, IpcReplyPayload, IpcRequest};

pub(crate) fn handle_inbound(app: &mut GuiApp, raw: &str) {
    let req: IpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            app.append_log(format!("ipc: bad request: {e}"));
            return;
        },
    };
    let id = req.id;
    let cmd = req.cmd.clone();
    tracing::debug!(id, cmd = %cmd, "ipc dispatch");
    match command::dispatch(app, id, &req.cmd, req.args) {
        CommandOutcome::Sync(result) => {
            let payload = command::reply_for_value(result);
            send_reply_payload(app, id, &payload);
        },
        CommandOutcome::Async => {},
    }
}

pub(crate) fn send_emit(app: &GuiApp, channel: &str, payload: &Value) {
    let script = ipc::emit_script(channel, payload);
    if let Some(win) = &app.settings_window {
        win.evaluate_script(&script);
    }
}

pub(crate) fn send_reply(app: &GuiApp, id: u64, payload: Value, ok: bool) {
    let body = if ok {
        IpcReplyPayload::ok(payload)
    } else {
        IpcReplyPayload::err(BridgeError::internal(payload.to_string()))
    };
    send_reply_payload(app, id, &body);
}

pub(crate) fn send_reply_payload(app: &GuiApp, id: u64, payload: &IpcReplyPayload) {
    let script = ipc::reply_script(id, payload);
    if let Some(win) = &app.settings_window {
        win.evaluate_script(&script);
    }
}

pub(crate) fn emit_proxy_stats(app: &GuiApp) {
    let value = crate::gui::server_json::proxy_stats_value();
    send_emit(app, "proxy.stats", &value);
}

pub(crate) fn emit_gateway_changed(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = json!({
        "state": gateway_state_str(&snap.gateway_status),
        "identity": crate::gui::server_json::identity_value(&snap),
        "verified_identity": crate::gui::server_json::identity_value(&snap),
        "lastProbeAtUnix": snap.last_probe_at_unix,
        "signedIn": snap.signed_in(),
    });
    send_emit(app, "gateway.changed", &value);
}

pub(crate) fn emit_host_changed(app: &GuiApp, host_id: &str) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::single_host_value(&snap, host_id);
    send_emit(app, "host.changed", &value);
}

pub(crate) fn emit_proxy_changed(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::local_proxy_value(&snap);
    send_emit(app, "proxy.changed", &value);
}

pub(crate) fn emit_sync_progress(app: &GuiApp, phase: &str, summary: Option<&str>) {
    let value = json!({
        "phase": phase,
        "summary": summary,
    });
    send_emit(app, "sync.progress", &value);
}

pub(crate) fn emit_state(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::snapshot_value(&snap);
    send_emit(app, "state.changed", &value);
}

pub(crate) fn emit_theme_changed(app: &GuiApp, theme: &str) {
    send_emit(app, "os.theme-changed", &json!({ "theme": theme }));
}

pub(crate) fn emit_error(app: &GuiApp, error: &BridgeError) {
    let value = serde_json::to_value(error).unwrap_or(Value::Null);
    send_emit(app, "error", &value);
}

fn gateway_state_str(status: &crate::gui::state::GatewayStatus) -> &'static str {
    match status {
        crate::gui::state::GatewayStatus::Unknown => "unknown",
        crate::gui::state::GatewayStatus::Probing => "probing",
        crate::gui::state::GatewayStatus::Reachable { .. } => "reachable",
        crate::gui::state::GatewayStatus::Unreachable { .. } => "unreachable",
    }
}

pub(crate) fn install_log_emitter(proxy: winit::event_loop::EventLoopProxy<UiEvent>) {
    crate::activity::activity_log().add_emit_hook(Box::new(move |entry| {
        let value = serde_json::to_value(entry).unwrap_or(Value::Null);
        let _ = proxy.send_event(UiEvent::IpcEmit {
            channel: "log",
            payload: value,
        });
    }));
}
