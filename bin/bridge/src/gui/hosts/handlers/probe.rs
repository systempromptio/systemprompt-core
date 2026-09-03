//! Host and proxy probe handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::{GuiApp, emit};
use crate::ids::HostId;
use crate::integration::{HostAppSnapshot, ProfileState, ProxyHealth};
use crate::proxy_probe;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

use serde_json::json;

use super::finish;

pub(crate) fn on_probe_requested(
    app: &GuiApp,
    host_id: &HostId,
    cause: ProbeCause,
    reply_to: ReplyId,
) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id.as_str(), "re-verify", reply_to)
    else {
        return;
    };
    if cause == ProbeCause::Manual {
        app.append_log(format!("[{host_id}] re-verifying profile and process"));
    } else if !app.state.mark_host_probing(host_id.as_str()) {
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Conflict,
                "probe already in flight",
            );
            emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    let host_id_owned = host_id.clone();
    let proxy = app.proxy.clone();
    let env = app.probe_env();
    app.ctx.spawn(async move {
        let Ok(snap) = tokio::task::spawn_blocking(move || Box::new(host.probe(&env))).await else {
            return;
        };
        proxy.send_event(UiEvent::Host(HostUiEvent::ProbeFinished {
            host_id: host_id_owned,
            cause,
            snapshot: snap,
            reply_to,
        }));
    });
}

pub(crate) fn on_probe_finished(
    app: &mut GuiApp,
    host_id: &HostId,
    cause: ProbeCause,
    snapshot: &HostAppSnapshot,
    reply_to: ReplyId,
) {
    let summary = describe_snapshot(snapshot, app.ctx.proxy.port());
    let prev = app
        .state
        .snapshot()
        .hosts
        .get(host_id.as_str())
        .and_then(|s| s.snapshot.clone());
    app.state
        .apply_host_snapshot(host_id.as_str(), snapshot.clone());
    app.refresh_ui();
    emit::emit_host_changed(app, host_id);
    let log_line = match cause {
        ProbeCause::Manual => Some(format!("[{host_id}] re-verify complete — {summary}")),
        ProbeCause::Tick => {
            state_change_line(host_id, prev.as_ref(), snapshot, app.ctx.proxy.port())
        },
    };
    if let Some(line) = log_line {
        app.append_log(line);
    }
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::single_host_value(&snap, host_id.as_str());
    if app.state.first_run_active() {
        crate::gui::first_run::handlers::on_probe_result(app, host_id, snapshot);
    }
    finish(app, Ok(json!({ "snapshot": value })), reply_to);
}

fn state_change_line(
    host_id: &HostId,
    prev: Option<&HostAppSnapshot>,
    next: &HostAppSnapshot,
    proxy_port: u16,
) -> Option<String> {
    let prev = prev?;
    let profile_changed =
        profile_state_kind(&prev.profile_state) != profile_state_kind(&next.profile_state);
    let process_changed = prev.host_running != next.host_running;
    if !profile_changed && !process_changed {
        return None;
    }
    Some(format!(
        "[{host_id}] state changed — {}",
        describe_snapshot(next, proxy_port)
    ))
}

const fn profile_state_kind(s: &ProfileState) -> &'static str {
    match s {
        ProfileState::Installed => "installed",
        ProfileState::Partial { .. } => "partial",
        ProfileState::Absent => "absent",
        ProfileState::Stale { .. } => "stale",
    }
}

fn describe_snapshot(snap: &HostAppSnapshot, proxy_port: u16) -> String {
    use crate::integration::{ProfileState, StaleReason};
    let profile = match &snap.profile_state {
        ProfileState::Installed => "profile installed".to_owned(),
        ProfileState::Partial { missing_required } => {
            format!("profile partial (missing: {})", missing_required.join(", "))
        },
        ProfileState::Absent => "profile not installed".to_owned(),
        ProfileState::Stale { reason } => match reason {
            StaleReason::LoopbackSecret => {
                "profile secret out of date (re-apply required)".to_owned()
            },
            StaleReason::ProxyPort => format!(
                "profile points at the wrong proxy port — this proxy is on {proxy_port} \
                 (re-apply required)"
            ),
        },
    };
    let process = if snap.host_running {
        "process running"
    } else {
        "process not running"
    };
    format!("{profile}, {process}")
}

pub(crate) fn on_proxy_probe_requested(app: &GuiApp, reply_to: ReplyId) {
    let url = app.state.first_configured_proxy_url();
    if !app.state.mark_proxy_probing() {
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Proxy,
                ErrorCode::Conflict,
                "proxy probe already in flight",
            );
            emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    let proxy = app.proxy.clone();
    app.ctx.spawn(async move {
        let Ok(health) =
            tokio::task::spawn_blocking(move || Box::new(proxy_probe::probe(url.as_deref()))).await
        else {
            return;
        };
        proxy.send_event(UiEvent::Host(HostUiEvent::ProxyProbeFinished {
            health,
            reply_to,
        }));
    });
}

pub(crate) fn on_proxy_probe_finished(app: &mut GuiApp, health: ProxyHealth, reply_to: ReplyId) {
    app.state.apply_proxy_health(health);
    app.refresh_ui();
    emit::emit_proxy_changed(app);
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::local_proxy_value(&snap);
    finish(app, Ok(json!({ "health": value })), reply_to);
}
