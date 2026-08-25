//! Handlers applying host-probe results to GUI state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, emit};
use crate::ids::HostId;
use crate::integration::{
    GeneratedProfile, HostAppSnapshot, ProfileGenInputs, ProfileState, ProxyHealth,
    find_host_by_id, proxy_probe,
};

pub(crate) fn on_probe_requested(
    app: &GuiApp,
    host_id: &HostId,
    cause: ProbeCause,
    reply_to: ReplyId,
) {
    let Some(host) = find_host_by_id(host_id.as_str()) else {
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
    app.runtime.spawn(async move {
        let Ok(snap) = tokio::task::spawn_blocking(move || Box::new(host.probe())).await else {
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
    let summary = describe_snapshot(snapshot);
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
        ProbeCause::Tick => state_change_line(host_id, prev.as_ref(), snapshot),
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
        describe_snapshot(next)
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

fn describe_snapshot(snap: &HostAppSnapshot) -> String {
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
                "profile points at the wrong proxy port — this proxy is on {} (re-apply required)",
                crate::proxy::resolved_port()
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
    app.runtime.spawn(async move {
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

pub(crate) fn on_profile_generate_requested(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    let Some(host) = find_host_by_id(host_id.as_str()) else {
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
    let host_id_owned = host_id.clone();
    let overrides = app.state.snapshot().host_model_protocols;
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = generate_profile_for(host, &overrides)
            .await
            .map_err(Arc::new);
        proxy.send_event(UiEvent::Host(HostUiEvent::ProfileGenerateFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

pub(crate) fn on_profile_generate_finished(
    app: &mut GuiApp,
    host_id: &HostId,
    result: Result<GeneratedProfile, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(p) => {
            app.append_log(format!(
                "[{host_id}] profile written: {} ({} bytes)",
                p.path, p.bytes
            ));
            let response = json!({ "path": p.path, "bytes": p.bytes });
            app.state.set_last_generated_profile(host_id.as_str(), p);
            Ok(response)
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
    emit::emit_host_changed(app, host_id);
    if app.state.first_run_active() {
        let error = bridge_result.as_ref().err().map(|e| e.message.clone());
        crate::gui::first_run::handlers::on_generate_result(app, host_id, error);
    }
    finish(app, bridge_result, reply_to);
}

fn needs_elevation_notice(host: &dyn crate::integration::HostApp) -> bool {
    #[cfg(target_os = "windows")]
    {
        host.config_format() == crate::integration::ConfigFormat::Reg
            && !crate::winproc::is_elevated()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = host;
        false
    }
}

pub(crate) fn on_profile_install_requested(
    app: &GuiApp,
    host_id: &HostId,
    path: String,
    reply_to: ReplyId,
) {
    let Some(host) = find_host_by_id(host_id.as_str()) else {
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
    if needs_elevation_notice(host) {
        app.append_log(format!(
            "[{host_id}] administrator approval is required to write the machine-wide Claude \
             policy (HKLM\\SOFTWARE\\Policies\\Claude). A Windows UAC prompt will appear — \
             approve it to continue."
        ));
    }
    let host_id_owned = host_id.clone();
    let path_clone = path.clone();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = match tokio::task::spawn_blocking(move || {
            host.install_profile(&path)
                .map(|()| path_clone)
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
        proxy.send_event(UiEvent::Host(HostUiEvent::ProfileInstallFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

pub(crate) fn on_profile_install_finished(
    app: &mut GuiApp,
    host_id: &HostId,
    result: Result<String, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let action = find_host_by_id(host_id.as_str()).map_or(
        "installed",
        crate::integration::host_app::HostApp::install_action_label,
    );
    let bridge_result = match result {
        Ok(path) => {
            app.append_log(format!("[{host_id}] {action}: {path}"));
            Ok(json!({ "path": path }))
        },
        Err(e) => {
            let (code, line) = match e.as_ref() {
                GuiError::Profile { source, .. }
                    if source.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    (ErrorCode::Unauthorized, format!("[{host_id}] {source}"))
                },
                _ => (
                    ErrorCode::Internal,
                    format!("[{host_id}] profile install failed: {e}"),
                ),
            };
            app.append_log(&line);
            Err(BridgeError::new(ErrorScope::Host, code, line))
        },
    };
    app.proxy
        .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
            host_id: host_id.clone(),
            cause: ProbeCause::Manual,
            reply_to: None,
        }));
    if app.state.first_run_active() {
        let error = bridge_result.as_ref().err().map(|e| e.message.clone());
        crate::gui::first_run::handlers::on_install_result(app, host_id, error);
    }
    finish(app, bridge_result, reply_to);
}

pub(crate) fn on_model_filter_set_requested(
    app: &GuiApp,
    host_id: &HostId,
    protocols: Option<Vec<String>>,
    reply_to: ReplyId,
) {
    if find_host_by_id(host_id.as_str()).is_none() {
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    }
    match &protocols {
        Some(list) if list.is_empty() => {
            app.append_log(format!("[{host_id}] model filter → all models"));
        },
        Some(list) => {
            app.append_log(format!("[{host_id}] model filter → {}", list.join(", ")));
        },
        None => app.append_log(format!("[{host_id}] model filter cleared (host default)")),
    }
    let host_id_owned = host_id.clone();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = push_model_filter(&host_id_owned, protocols.as_deref())
            .await
            .map_err(Arc::new);
        proxy.send_event(UiEvent::Host(HostUiEvent::ModelFilterSetFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

async fn push_model_filter(host_id: &HostId, protocols: Option<&[String]>) -> GuiResult<()> {
    let cfg = config::load();
    let gateway_base = config::gateway_url_or_default(&cfg);
    let bearer =
        crate::auth::cache::read_valid(&gateway_base).ok_or_else(|| GuiError::Profile {
            context: "model filter".into(),
            source: std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "not signed in; cannot update host model filter",
            ),
        })?;
    GatewayClient::new(gateway_base)
        .set_host_model_filter(bearer.token.expose(), host_id.as_str(), protocols)
        .await
        .map_err(|e| GuiError::Profile {
            context: "host model filter".into(),
            source: std::io::Error::other(e.to_string()),
        })
}

pub(crate) fn on_model_filter_set_finished(
    app: &GuiApp,
    host_id: &HostId,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(format!("[{host_id}] model filter saved; re-syncing"));
            app.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            Ok(json!({ "host_id": host_id }))
        },
        Err(e) => {
            let line = format!("[{host_id}] model filter update failed: {e}");
            app.append_log(&line);
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    finish(app, bridge_result, reply_to);
}

async fn generate_profile_for(
    host: &'static dyn crate::integration::HostApp,
    overrides: &std::collections::BTreeMap<String, Vec<String>>,
) -> GuiResult<GeneratedProfile> {
    let cfg = config::load();

    // Why: a proxy that had to move off the default port is still perfectly
    // usable, and `loopback_origin` finds it. Refusing on a missing in-process
    // handle used to make a GUI that lost the port race unable to write any
    // profile at all.
    let gateway_base_url = crate::proxy::loopback_origin();

    // Why: a foreign install on our port must still refuse — writing *our*
    // loopback secret against *their* port produces exactly the 403 this
    // profile is supposed to prevent.
    let port = crate::proxy::resolved_port();
    if let proxy_probe::PeerIdentity::Foreign(who) = proxy_probe::probe_identity(port) {
        return Err(GuiError::Profile {
            context: "proxy port held by another install".into(),
            source: std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!(
                    "127.0.0.1:{port} is served by a different {} install ({}); a profile written \
                     now would authenticate against the wrong proxy",
                    crate::brand::brand().app_name,
                    who.config_dir
                ),
            ),
        });
    }

    let loopback_secret = crate::proxy::secret::for_profile()
        .map(crate::ids::LoopbackSecret::into_inner)
        .map_err(|e| GuiError::Profile {
            context: "loopback secret".into(),
            source: e,
        })?;

    let gateway_base = config::gateway_url_or_default(&cfg);
    let server_profile = GatewayClient::new(gateway_base)
        .fetch_bridge_profile()
        .await?;

    let surfaces = crate::integration::host_app::effective_surfaces(
        host.id(),
        host.accepted_surfaces(),
        overrides,
    );
    let view = crate::integration::host_app::host_model_view(&server_profile.providers, &surfaces);
    let models = view.compatible_models;

    let mut headers = std::collections::BTreeMap::new();
    if !surfaces.is_empty() {
        headers.insert(
            systemprompt_identifiers::headers::INFERENCE_PROTOCOL.to_owned(),
            surfaces
                .iter()
                .map(|s| s.as_tag())
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    let inputs = ProfileGenInputs {
        gateway_base_url,
        api_key: loopback_secret,
        models,
        organization_uuid: server_profile.organization_uuid,
        headers,
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
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            emit::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
