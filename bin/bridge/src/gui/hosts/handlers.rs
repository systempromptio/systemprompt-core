use std::sync::Arc;

use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, ipc_runtime};
use crate::integration::{
    GeneratedProfile, HostAppSnapshot, ProfileGenInputs, ProfileState, ProxyHealth,
    find_host_by_id, proxy_probe,
};

pub(crate) fn on_probe_requested(
    app: &mut GuiApp,
    host_id: &str,
    cause: ProbeCause,
    reply_to: ReplyId,
) {
    let Some(host) = find_host_by_id(host_id) else {
        if cause == ProbeCause::Manual {
            app.append_log(format!("probe requested for unknown host '{host_id}'"));
        }
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    };
    if cause == ProbeCause::Manual && !app.state.is_host_enabled(host_id) {
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::Conflict,
            format!("host '{host_id}' is disabled"),
        );
        finish(app, Err(err), reply_to);
        return;
    }
    if !app.state.mark_host_probing(host_id) {
        if cause == ProbeCause::Manual {
            app.append_log(format!("[{host_id}] re-verify already in flight"));
        }
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Conflict,
                "probe already in flight",
            );
            ipc_runtime::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    if cause == ProbeCause::Manual {
        app.append_log(format!("[{host_id}] re-verifying profile and process"));
    }
    let host_id_owned = host_id.to_string();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let snap = match tokio::task::spawn_blocking(move || Box::new(host.probe())).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = proxy.send_event(UiEvent::Host(HostUiEvent::ProbeFinished {
            host_id: host_id_owned,
            cause,
            snapshot: snap,
            reply_to,
        }));
    });
}

pub(crate) fn on_probe_finished(
    app: &mut GuiApp,
    host_id: &str,
    cause: ProbeCause,
    snapshot: HostAppSnapshot,
    reply_to: ReplyId,
) {
    let summary = describe_snapshot(&snapshot);
    let prev = app
        .state
        .snapshot()
        .hosts
        .get(host_id)
        .and_then(|s| s.snapshot.clone());
    app.state.apply_host_snapshot(host_id, snapshot.clone());
    let _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProxyProbeRequested {
            reply_to: None,
        }));
    app.refresh_ui();
    ipc_runtime::emit_host_changed(app, host_id);
    let log_line = match cause {
        ProbeCause::Manual => Some(format!("[{host_id}] re-verify complete — {summary}")),
        ProbeCause::Tick => state_change_line(host_id, prev.as_ref(), &snapshot),
    };
    if let Some(line) = log_line {
        app.append_log(line);
    }
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::single_host_value(&snap, host_id);
    finish(app, Ok(json!({ "snapshot": value })), reply_to);
}

fn state_change_line(
    host_id: &str,
    prev: Option<&HostAppSnapshot>,
    next: &HostAppSnapshot,
) -> Option<String> {
    let prev = prev?;
    let profile_changed = profile_state_kind(&prev.profile_state) != profile_state_kind(&next.profile_state);
    let process_changed = prev.host_running != next.host_running;
    if !profile_changed && !process_changed {
        return None;
    }
    Some(format!(
        "[{host_id}] state changed — {}",
        describe_snapshot(next)
    ))
}

fn profile_state_kind(s: &ProfileState) -> &'static str {
    match s {
        ProfileState::Installed => "installed",
        ProfileState::Partial { .. } => "partial",
        ProfileState::Absent => "absent",
    }
}

fn describe_snapshot(snap: &HostAppSnapshot) -> String {
    use crate::integration::ProfileState;
    let profile = match &snap.profile_state {
        ProfileState::Installed => "profile installed".to_string(),
        ProfileState::Partial { missing_required } => {
            format!("profile partial (missing: {})", missing_required.join(", "))
        },
        ProfileState::Absent => "profile not installed".to_string(),
    };
    let process = if snap.host_running {
        "process running"
    } else {
        "process not running"
    };
    format!("{profile}, {process}")
}

pub(crate) fn on_proxy_probe_requested(app: &mut GuiApp, reply_to: ReplyId) {
    let url = app.state.first_configured_proxy_url();
    if !app.state.mark_proxy_probing() {
        if let Some(id) = reply_to {
            let err = BridgeError::new(
                ErrorScope::Proxy,
                ErrorCode::Conflict,
                "proxy probe already in flight",
            );
            ipc_runtime::send_reply_payload(app, id, &IpcReplyPayload::err(err));
        }
        return;
    }
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let health =
            match tokio::task::spawn_blocking(move || Box::new(proxy_probe::probe(url.as_deref())))
                .await
            {
                Ok(h) => h,
                Err(_) => return,
            };
        let _ = proxy.send_event(UiEvent::Host(HostUiEvent::ProxyProbeFinished {
            health,
            reply_to,
        }));
    });
}

pub(crate) fn on_proxy_probe_finished(app: &mut GuiApp, health: ProxyHealth, reply_to: ReplyId) {
    app.state.apply_proxy_health(health);
    app.refresh_ui();
    ipc_runtime::emit_proxy_changed(app);
    let snap = app.state.snapshot();
    let value = crate::gui::server_json::local_proxy_value(&snap);
    finish(app, Ok(json!({ "health": value })), reply_to);
}

pub(crate) fn on_profile_generate_requested(app: &mut GuiApp, host_id: &str, reply_to: ReplyId) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("generate requested for unknown host '{host_id}'"));
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    };
    app.append_log(format!("Generating profile for {}…", host.display_name()));
    let host_id_owned = host_id.to_string();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = generate_profile_for(host).await.map_err(Arc::new);
        let _ = proxy.send_event(UiEvent::Host(HostUiEvent::ProfileGenerateFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

pub(crate) fn on_profile_generate_finished(
    app: &mut GuiApp,
    host_id: &str,
    result: Result<GeneratedProfile, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(p) => {
            app.state
                .set_last_generated_profile(host_id, p.path.clone());
            app.append_log(format!(
                "[{host_id}] profile written: {} ({} bytes)",
                p.path, p.bytes
            ));
            Ok(json!({ "path": p.path, "bytes": p.bytes }))
        },
        Err(e) => {
            let line = format!("[{host_id}] profile generation failed: {e}");
            app.append_log(&line);
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    app.refresh_ui();
    ipc_runtime::emit_host_changed(app, host_id);
    finish(app, bridge_result, reply_to);
}

pub(crate) fn on_profile_install_requested(
    app: &mut GuiApp,
    host_id: &str,
    path: String,
    reply_to: ReplyId,
) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("install requested for unknown host '{host_id}'"));
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    };
    app.append_log(format!("[{host_id}] installing {path}…"));
    let host_id_owned = host_id.to_string();
    let path_clone = path.clone();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = match tokio::task::spawn_blocking(move || {
            host.install_profile(&path)
                .map(|_| path_clone)
                .map_err(|e| GuiError::Profile {
                    context: "host install_profile".into(),
                    source: e,
                })
                .map_err(Arc::new)
        })
        .await
        {
            Ok(r) => r,
            Err(join_err) => Err(Arc::new(GuiError::Io(std::io::Error::other(format!(
                "profile install task join: {join_err}"
            ))))),
        };
        let _ = proxy.send_event(UiEvent::Host(HostUiEvent::ProfileInstallFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

pub(crate) fn on_profile_install_finished(
    app: &mut GuiApp,
    host_id: &str,
    result: Result<String, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let action = find_host_by_id(host_id)
        .map(|h| h.install_action_label())
        .unwrap_or("installed");
    let bridge_result = match result {
        Ok(path) => {
            app.append_log(format!("[{host_id}] {action}: {path}"));
            Ok(json!({ "path": path }))
        },
        Err(e) => {
            let line = format!("[{host_id}] profile install failed: {e}");
            app.append_log(&line);
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    let _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
            host_id: host_id.to_string(),
            cause: ProbeCause::Manual,
            reply_to: None,
        }));
    finish(app, bridge_result, reply_to);
}

async fn generate_profile_for(
    host: &'static dyn crate::integration::HostApp,
) -> GuiResult<GeneratedProfile> {
    let cfg = config::load();

    let port = crate::proxy::handle()
        .map(|h| h.port)
        .ok_or_else(|| GuiError::Profile {
            context: "proxy not running".into(),
            source: std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!(
                    "local proxy is not listening on port {}; cannot generate a profile that points to a dead endpoint",
                    crate::proxy::DEFAULT_PROXY_PORT
                ),
            ),
        })?;

    let loopback_secret = crate::proxy::secret::for_profile()
        .map(crate::ids::LoopbackSecret::into_inner)
        .map_err(|e| GuiError::Profile {
            context: "loopback secret".into(),
            source: e,
        })?;

    let gateway_base = config::gateway_url_or_default(&cfg);
    let server_profile = GatewayClient::new(gateway_base).fetch_cowork_profile().await?;

    let models = if server_profile.models.is_empty() {
        crate::integration::claude_desktop::default_models()
    } else {
        server_profile.models
    };

    let inputs = ProfileGenInputs {
        gateway_base_url: format!("http://127.0.0.1:{port}"),
        api_key: loopback_secret,
        models,
        organization_uuid: server_profile.organization_uuid,
    };
    host.generate_profile(&inputs)
        .map_err(|e| GuiError::Profile {
            context: "host generate_profile".into(),
            source: e,
        })
}

fn finish(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            ipc_runtime::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            ipc_runtime::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    ipc_runtime::send_reply_payload(app, id, &payload);
}
