//! Host and proxy probe handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::hosts::state::ProbeSeq;
use crate::gui::{GuiApp, emit};
use crate::host_sync::HostSync;
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
    }
    let Some(seq) = app
        .state
        .begin_host_probe(host_id.as_str(), cause == ProbeCause::Tick)
    else {
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Conflict,
                "probe already in flight",
            );
            emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    };
    let host_id_owned = host_id.clone();
    let proxy = app.proxy.clone();
    let env = app.probe_env();
    app.ctx.spawn(async move {
        let snap = match tokio::task::spawn_blocking(move || Box::new(host.probe(&env))).await {
            Ok(snap) => snap,
            Err(e) => {
                proxy.send_event(UiEvent::Host(HostUiEvent::ProbeFailed {
                    host_id: Some((host_id_owned, seq)),
                    error: format!("host probe task failed: {e}"),
                    reply_to,
                }));
                return;
            },
        };
        proxy.send_event(UiEvent::Host(HostUiEvent::ProbeFinished {
            host_id: host_id_owned,
            seq,
            cause,
            snapshot: snap,
            reply_to,
        }));
    });
}

#[derive(Clone, Copy)]
pub(crate) struct ProbeResult<'a> {
    pub host_id: &'a HostId,
    pub seq: ProbeSeq,
    pub cause: ProbeCause,
    pub snapshot: &'a HostAppSnapshot,
}

pub(crate) fn on_probe_finished(app: &mut GuiApp, result: &ProbeResult<'_>, reply_to: ReplyId) {
    let ProbeResult {
        host_id,
        seq,
        cause,
        snapshot,
    } = *result;
    let summary = describe_snapshot(snapshot, app.ctx.proxy.port());
    let prev = app
        .state
        .snapshot()
        .hosts
        .get(host_id.as_str())
        .and_then(|s| s.snapshot.clone());
    if !app
        .state
        .apply_host_snapshot(host_id.as_str(), seq, snapshot.clone())
    {
        tracing::debug!(host_id = %host_id, "superseded host probe result discarded");
        finish(app, Ok(json!({ "superseded": true })), reply_to);
        return;
    }
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
    if cause == ProbeCause::Tick && cowork_session_now_available(app, host_id) {
        app.append_log(format!(
            "[{host_id}] Cowork session detected — syncing to enable the org plugins"
        ));
        app.proxy
            .send_event(UiEvent::SyncRequested { reply_to: None });
    }
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::single_host_value(&snap, host_id.as_str());
    if app.state.first_run_active() {
        crate::gui::first_run::handlers::on_probe_result(app, host_id, snapshot);
    }
    finish(app, Ok(json!({ "snapshot": value })), reply_to);
}

fn cowork_session_now_available(app: &GuiApp, host_id: &HostId) -> bool {
    if host_id.as_str() != crate::integration::cowork_plugins::CoworkSync.host_id() {
        return false;
    }
    let snap = app.state.snapshot();
    if snap.sync_in_flight {
        return false;
    }
    let outstanding = snap
        .last_sync_report
        .as_ref()
        .is_some_and(|report| report.host_warnings.iter().any(|w| w.host_id == *host_id));
    outstanding
        && matches!(
            crate::integration::cowork_plugins::resolve_target(),
            Ok(Some(_))
        )
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
        ProfileState::Unverifiable { .. } => "unverifiable",
    }
}

fn describe_snapshot(snap: &HostAppSnapshot, proxy_port: u16) -> String {
    use crate::integration::{ProfileState, StaleReason};
    let profile = match &snap.profile_state {
        ProfileState::Installed => "profile installed".to_owned(),
        ProfileState::Partial { missing_required } => {
            format!("profile partial (missing: {})", missing_required.join(", "))
        },
        ProfileState::Unverifiable { reason } => format!("profile unverifiable ({reason})"),
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
    let process = match snap.host_running {
        Some(true) => "process running",
        Some(false) => "process not running",
        None => "process state unknown (enumeration failed)",
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
        let health =
            match tokio::task::spawn_blocking(move || Box::new(proxy_probe::probe(url.as_deref())))
                .await
            {
                Ok(health) => health,
                Err(e) => {
                    proxy.send_event(UiEvent::Host(HostUiEvent::ProbeFailed {
                        host_id: None,
                        error: format!("proxy probe task failed: {e}"),
                        reply_to,
                    }));
                    return;
                },
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

pub(crate) fn on_probe_failed(
    app: &mut GuiApp,
    host_id: Option<&(HostId, ProbeSeq)>,
    error: &str,
    reply_to: ReplyId,
) {
    app.state
        .finish_failed_probe(host_id.map(|(id, seq)| (id.as_str(), *seq)));
    app.append_log_error(error);
    if let Some(id) = reply_to {
        emit::send_reply_payload(app, id, &IpcReplyPayload::err(BridgeError::internal(error)));
    }
    app.refresh_ui();
}
