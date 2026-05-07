use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, emit, ipc_runtime, window};
use crate::integration::find_host_by_id;

pub(crate) fn on_uninstall(app: &mut GuiApp, host_id: &str, reply_to: ReplyId) {
    if let Some(err) = enabled_guard(app, host_id) {
        finish(app, Err(err), reply_to);
        return;
    }
    let result = match find_host_by_id(host_id) {
        Some(host) => {
            app.append_log(format!(
                "uninstall requested for {} (not yet implemented; remove systemprompt-bridge keys \
                 manually)",
                host.display_name()
            ));
            Ok(json!({ "queued": true }))
        },
        None => {
            app.append_log(format!("uninstall: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
    };
    finish(app, result, reply_to);
}

fn enabled_guard(app: &GuiApp, host_id: &str) -> Option<BridgeError> {
    if app.state.is_host_enabled(host_id) {
        None
    } else {
        Some(BridgeError::new(
            ErrorScope::Host,
            ErrorCode::Conflict,
            format!("host '{host_id}' is disabled"),
        ))
    }
}

pub(crate) fn on_open_config(app: &mut GuiApp, host_id: &str, reply_to: ReplyId) {
    if let Some(err) = enabled_guard(app, host_id) {
        finish(app, Err(err), reply_to);
        return;
    }
    let result = match find_host_by_id(host_id) {
        Some(host) => {
            let snapshot = host.probe();
            if let Some(path) = snapshot.profile_source.as_ref() {
                window::open_path(Path::new(path));
                app.append_log(format!(
                    "opened config for {} at {path}",
                    host.display_name()
                ));
                Ok(json!({ "path": path }))
            } else {
                let msg = format!(
                    "open-config: no resolved config path for {}",
                    host.display_name()
                );
                app.append_log(&msg);
                Err(BridgeError::new(ErrorScope::Host, ErrorCode::NotFound, msg))
            }
        },
        None => {
            app.append_log(format!("open-config: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
    };
    finish(app, result, reply_to);
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_set_enabled_host_requested(
    app: &mut GuiApp,
    host_id: String,
    enabled: bool,
    reply_to: ReplyId,
) {
    if find_host_by_id(&host_id).is_none() {
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    }
    let proxy = app.proxy.clone();
    let host_id_async = host_id.clone();
    app.runtime.spawn(async move {
        let result = post_enabled_host(&host_id_async, enabled).await;
        let _ = proxy.send_event(UiEvent::SetEnabledHostFinished {
            host_id: host_id_async,
            enabled,
            result: result.map_err(GuiError::from).map_err(Arc::new),
            reply_to,
        });
    });
}

async fn post_enabled_host(host_id: &str, enabled: bool) -> Result<(), std::io::Error> {
    use crate::config;
    use crate::gateway::GatewayClient;
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let bearer = match crate::auth::cache::read_valid() {
        Some(out) => out.token,
        None => {
            return Err(std::io::Error::other(
                "no valid auth credential available; log in first",
            ));
        },
    };
    let client = GatewayClient::new(gateway);
    client
        .set_enabled_host(bearer.expose(), host_id, enabled)
        .await
        .map_err(|e| {
            tracing::error!(host_id = %host_id, enabled, error = %e, "set enabled host failed");
            std::io::Error::other(e.to_string())
        })
}

pub(crate) fn on_set_enabled_host_finished(
    app: &mut GuiApp,
    host_id: String,
    enabled: bool,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(format!(
                "[{host_id}] agent {}",
                if enabled { "enabled" } else { "disabled" }
            ));
            // Trigger immediate sync so the manifest re-pulls with the new
            // enabled_hosts list and dispatches emitters accordingly.
            let _ = app
                .proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            Ok(json!({ "hostId": host_id, "enabled": enabled, "changed": true }))
        },
        Err(e) => Err(BridgeError::new(
            ErrorScope::Host,
            ErrorCode::Internal,
            format!("set enabled host: {e}"),
        )),
    };
    finish(app, bridge_result, reply_to);
}

pub(crate) fn on_setup_complete(app: &mut GuiApp) {
    app.state.set_agents_onboarded(true);
    app.append_log("setup marked complete by user");
    emit::emit_state(app);
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
