//! Outbound GUI event and reply emission to the webview channel.
//!
//! A reply is addressed to the webview mount that issued the request; a mount
//! that has since been replaced (page reload) never receives it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::{Value, json};

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::wire::ipc::{self, BridgeError, IpcReplyPayload, ReplyTarget};

pub(crate) fn deliver(app: &GuiApp, script: &str) -> bool {
    let Some(win) = &app.settings_window else {
        return false;
    };
    match win.evaluate_script(script) {
        Ok(()) => true,
        Err(e) => {
            app.append_log_error(format!("webview delivery failed: {e}"));
            false
        },
    }
}

pub(crate) fn send_emit(app: &GuiApp, channel: &str, payload: &Value) {
    deliver(app, &ipc::emit_script(channel, payload));
}

pub(crate) fn send_reply_payload(app: &GuiApp, target: ReplyTarget, payload: &IpcReplyPayload) {
    if app.stale_mount(target.mount) {
        tracing::debug!(
            mount = target.mount,
            id = target.id,
            "reply dropped: webview mount replaced"
        );
        return;
    }
    deliver(app, &ipc::reply_script(target, payload));
}

pub(crate) fn finish<T: Serialize>(
    app: &GuiApp,
    reply_to: ReplyId,
    result: Result<T, BridgeError>,
) {
    let result = result.and_then(|value| {
        serde_json::to_value(value)
            .map_err(|e| BridgeError::internal(format!("reply encode failed: {e}")))
    });
    let Some(target) = reply_to else {
        if let Err(err) = result {
            emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(value) => IpcReplyPayload::ok(value),
        Err(err) => {
            emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    send_reply_payload(app, target, &payload);
}

pub(crate) fn emit_proxy_stats(app: &GuiApp) {
    let value = crate::gui::server_json::proxy_stats_value(&app.ctx.proxy);
    send_emit(app, "proxy.stats", &value);
}

pub(crate) fn emit_gateway_changed(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = json!({
        "state": snap.gateway_status.code(),
        "identity": crate::gui::server_json::identity_value(&snap),
        "verified_identity": crate::gui::server_json::identity_value(&snap),
        "lastProbeAtUnix": snap.last_probe_at_unix,
        "signedIn": snap.signed_in(),
    });
    send_emit(app, "gateway.changed", &value);
}

pub(crate) fn emit_host_changed(app: &mut GuiApp, host_id: &crate::ids::HostId) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::single_host_value(&snap, host_id.as_str());
    send_emit(app, "host.changed", &value);
    emit_state(app);
}

pub(crate) fn emit_proxy_changed(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::local_proxy_value(&snap);
    send_emit(app, "proxy.changed", &value);
}

pub(crate) fn emit_mcp_changed(app: &GuiApp) {
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::mcp_auth_value(&snap);
    send_emit(app, "mcp.changed", &value);
}

pub(crate) fn emit_sync_progress(app: &GuiApp, phase: &str, summary: Option<&str>) {
    let value = json!({
        "phase": phase,
        "summary": summary,
    });
    send_emit(app, "sync.progress", &value);
}

pub(crate) fn emit_sync_step(app: &GuiApp, step: &crate::progress::SyncProgress) {
    let value = json!({
        "phase": step.phase,
        "item": step.item,
        "current": step.current,
        "total": step.total,
        "detail": step.label(),
    });
    send_emit(app, "sync.progress", &value);
}

pub(crate) fn emit_first_run_progress(app: &GuiApp) {
    let snap = app.state.snapshot();
    let payload = crate::gui::first_run::serde::build(&snap.first_run);
    match serde_json::to_value(&payload) {
        Ok(value) => send_emit(app, "setup.progress", &value),
        Err(e) => tracing::warn!(error = %e, "first-run progress serialize failed"),
    }
}

pub(crate) fn emit_state(app: &mut GuiApp) {
    let snap = app.state.snapshot();
    let payload = crate::gui::server_json::state_payload(&snap, &app.ctx.proxy);
    let value = match serde_json::to_value(&payload) {
        Ok(value) => value,
        Err(e) => {
            app.append_log_error(format!("serialize state: {e}"));
            return;
        },
    };
    let semantic = match payload.semantic_value() {
        Ok(value) => value,
        Err(e) => {
            app.append_log_error(format!("compare state: {e}"));
            return;
        },
    };
    if app.last_semantic_state.as_ref() == Some(&semantic) {
        return;
    }
    if deliver(app, &ipc::emit_script("state.changed", &value)) {
        app.last_semantic_state = Some(semantic);
    }
}

pub(crate) fn emit_theme_changed(app: &GuiApp, theme: &str) {
    send_emit(app, "os.theme-changed", &json!({ "theme": theme }));
}

pub(crate) fn emit_error(app: &GuiApp, error: &BridgeError) {
    let value = serde_json::to_value(error).unwrap_or(Value::Null);
    send_emit(app, "error", &value);
}

pub(crate) fn install_log_emitter(
    activity: &crate::activity::ActivityLog,
    proxy: crate::gui::UiEventProxy,
) {
    activity.add_emit_hook(Box::new(move |entry| {
        let value = serde_json::to_value(entry).unwrap_or(Value::Null);
        proxy.send_event(UiEvent::IpcEmit {
            channel: "log",
            payload: value,
        });
    }));
}
