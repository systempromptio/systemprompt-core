use std::path::Path;

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, ipc_runtime, window};
use crate::integration::find_host_by_id;

pub(crate) fn on_uninstall(app: &mut GuiApp, host_id: &str, reply_to: ReplyId) {
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

pub(crate) fn on_open_config(app: &mut GuiApp, host_id: &str, reply_to: ReplyId) {
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

pub(crate) fn on_setup_complete(app: &mut GuiApp) {
    app.state.set_agents_onboarded(true);
    app.append_log("setup marked complete by user");
    ipc_runtime::emit_state(app);
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
