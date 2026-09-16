//! Inbound GUI IPC message parsing and dispatch to command handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::GuiApp;
use crate::gui::command::{self, CommandOutcome};
use crate::gui::emit::send_reply_payload;
use crate::wire::ipc::IpcRequest;

pub(crate) fn handle_inbound(app: &mut GuiApp, raw: &str) {
    let req: IpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            app.append_log_error(format!("ipc: bad request: {e}"));
            return;
        },
    };
    let target = req.reply_target();
    app.note_mount(target.mount);
    tracing::debug!(id = target.id, mount = target.mount, cmd = %req.cmd, "ipc dispatch");
    match command::dispatch(app, target, &req.cmd, &req.args) {
        CommandOutcome::Sync(result) => {
            let payload = command::reply_for_value(result);
            send_reply_payload(app, target, &payload);
        },
        CommandOutcome::Async => {},
    }
}
