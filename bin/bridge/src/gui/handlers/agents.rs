//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, emit, window};
use crate::integration::find_host_by_id;

pub(crate) fn on_uninstall(app: &GuiApp, host_id: &str, reply_to: ReplyId) {
    let result = find_host_by_id(host_id).map_or_else(
        || {
            app.append_log(format!("uninstall: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| {
            app.append_log(format!(
                "uninstall requested for {} (not yet implemented; remove {} keys \
                 manually)",
                host.display_name(),
                crate::brand::brand().binary_name
            ));
            Ok(json!({ "queued": true }))
        },
    );
    finish(app, result, reply_to);
}

pub(crate) fn on_open_config(app: &GuiApp, host_id: &str, reply_to: ReplyId) {
    let result = find_host_by_id(host_id).map_or_else(
        || {
            app.append_log(format!("open-config: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| {
            let snapshot = host.probe();
            snapshot.profile_source.as_ref().map_or_else(
                || {
                    let msg = format!(
                        "open-config: no resolved config path for {}",
                        host.display_name()
                    );
                    app.append_log(&msg);
                    Err(BridgeError::new(ErrorScope::Host, ErrorCode::NotFound, msg))
                },
                |path| {
                    window::open_path(Path::new(path));
                    app.append_log(format!(
                        "opened config for {} at {path}",
                        host.display_name()
                    ));
                    Ok(json!({ "path": path }))
                },
            )
        },
    );
    finish(app, result, reply_to);
}

pub(crate) fn on_open(app: &GuiApp, host_id: &str, reply_to: ReplyId) {
    let result = find_host_by_id(host_id).map_or_else(
        || {
            app.append_log(format!("open: unknown host {host_id}"));
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::NotFound,
                format!("unknown host: {host_id}"),
            ))
        },
        |host| match host.open() {
            Ok(()) => {
                app.append_log(format!("opened host {}", host.display_name()));
                Ok(json!({}))
            },
            Err(err) => {
                let msg = format!("open host {} failed: {err}", host.display_name());
                app.append_log(&msg);
                Err(BridgeError::new(ErrorScope::Host, ErrorCode::Internal, msg))
            },
        },
    );
    finish(app, result, reply_to);
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
