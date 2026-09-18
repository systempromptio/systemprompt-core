//! Inbound GUI IPC message parsing and dispatch to command handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::GuiApp;
use crate::gui::command::{self, CommandOutcome};
use crate::gui::emit::send_reply_payload;
use crate::wire::ipc::{
    BridgeError, ErrorCode, ErrorScope, IpcEnvelopeHead, IpcReplyPayload, IpcRequest, ReplyTarget,
};

pub(crate) fn handle_inbound(app: &mut GuiApp, raw: &str) {
    let req: IpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            reject_unparsed(app, raw, &e.to_string());
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

fn reject_unparsed(app: &mut GuiApp, raw: &str, reason: &str) {
    let message = format!("ipc: bad request: {reason}");
    app.append_log_error(message.clone());
    let Some((id, sent_mount)) =
        IpcEnvelopeHead::of(raw).and_then(|head| Some((head.id?, head.mount)))
    else {
        return;
    };
    if let Some(mount) = sent_mount {
        app.note_mount(mount);
    }
    let mount = sent_mount.or(app.current_mount).unwrap_or_default();
    let error = BridgeError::new(ErrorScope::Internal, ErrorCode::InvalidFormat, message);
    send_reply_payload(app, ReplyTarget { mount, id }, &IpcReplyPayload::err(error));
}
