//! Profile generation and installation handlers.
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
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope};
use crate::gui::{GuiApp, emit};
use crate::ids::HostId;
use crate::integration::{GeneratedProfile, ProfileGenInputs, find_host_by_id};

use super::finish;

pub(crate) fn on_profile_generate_requested(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    let Some(host) = find_host_by_id(host_id.as_str()) else {
        app.append_log_error(format!("generate requested for unknown host '{host_id}'"));
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
    let loopback = app.ctx.proxy.loopback().clone();
    let install_id = app.ctx.install_id().clone();
    app.ctx.spawn(async move {
        let result = generate_profile_for(host, &loopback, &install_id, &overrides)
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
            app.append_log_error(&line);
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
        app.append_log_error(format!("install requested for unknown host '{host_id}'"));
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
    app.ctx.spawn(async move {
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
            app.append_log_error(&line);
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

async fn generate_profile_for(
    host: &'static dyn crate::integration::HostApp,
    loopback: &crate::proxy::LoopbackEndpoint,
    install_id: &crate::proxy::identity::InstallId,
    overrides: &std::collections::BTreeMap<String, Vec<String>>,
) -> GuiResult<GeneratedProfile> {
    let cfg = config::load();

    // Why: a proxy that had to move off the default port is still perfectly
    // usable, and the endpoint names it. Refusing on a missing in-process
    // handle used to make a GUI that lost the port race unable to write any
    // profile at all.
    let gateway_base_url = loopback.origin();

    // Why: a foreign install on our port must still refuse — writing *our*
    // loopback secret against *their* port produces exactly the 403 this
    // profile is supposed to prevent.
    let port = loopback.port();
    if let crate::proxy::peer::PeerIdentity::Foreign(who) =
        crate::proxy::peer::probe_identity(port, install_id)
    {
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

    let loopback_secret = loopback
        .secret()
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
