//! Profile generation and installation handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::{GuiApp, emit};
use crate::ids::HostId;
use crate::integration::{GeneratedProfile, find_host_by_id};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::finish;

pub(crate) fn on_profile_generate_requested(app: &GuiApp, host_id: &HostId, reply_to: ReplyId) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id.as_str(), "repair", reply_to)
    else {
        return;
    };
    app.append_log(format!("Generating profile for {}…", host.display_name()));
    let host_id_owned = host_id.clone();
    let overrides = app.state.snapshot().host_model_protocols;
    let proxy = app.proxy.clone();
    let bridge = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let result = generate_profile_for(host, &bridge, &overrides)
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
    let Some(host) = crate::gui::hosts::resolve::resolve_or_reply(
        app,
        host_id.as_str(),
        "install profile",
        reply_to,
    ) else {
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
    bridge: &crate::context::BridgeContext,
    overrides: &std::collections::BTreeMap<String, Vec<String>>,
) -> GuiResult<GeneratedProfile> {
    // Why: the inputs come from `integration::reapply`, which is also what
    // `install --apply` and `login` use. One builder is what stops the CLI
    // repair paths and this button writing subtly different profiles.
    let inputs = crate::integration::reapply::build_profile_inputs(bridge, host, overrides)
        .await
        .map_err(|e| GuiError::Profile {
            context: "profile inputs".into(),
            source: e,
        })?;
    host.generate_profile(&inputs)
        .map_err(|e| GuiError::Profile {
            context: "host generate_profile".into(),
            source: e,
        })
}
