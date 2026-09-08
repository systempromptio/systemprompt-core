//! The reply every unit-returning auth handler sends, and the runtime-config
//! swap every credential change performs before the UI reloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::BridgeError;

impl GuiApp {
    // Why: every credential change must swap the proxy's runtime config before
    // the UI reloads; a swap that fails is the reply, and the caller returns.
    pub(crate) fn reload_runtime_or_fail(&self, reply_to: ReplyId) -> bool {
        match self.ctx.proxy.reload_runtime_config() {
            Ok(()) => true,
            Err(e) => {
                self.append_log_error(e.to_string());
                finish_unit(self, Err(BridgeError::internal(e.to_string())), reply_to);
                false
            },
        }
    }
}

pub(crate) fn finish_unit(app: &GuiApp, result: Result<(), BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(()) => crate::wire::ipc::IpcReplyPayload::ok(json!({})),
        Err(err) => {
            emit::emit_error(app, &err);
            crate::wire::ipc::IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
