//! Handlers applying host-probe results to GUI state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod model_filter;
mod probe;
mod profile;

pub(crate) use model_filter::{on_model_filter_set_finished, on_model_filter_set_requested};
pub(crate) use probe::{
    on_probe_finished, on_probe_requested, on_proxy_probe_finished, on_proxy_probe_requested,
};
pub(crate) use profile::{
    on_profile_generate_finished, on_profile_generate_requested, on_profile_install_finished,
    on_profile_install_requested,
};

use crate::gui::events::ReplyId;
use crate::gui::ipc::{BridgeError, IpcReplyPayload};
use crate::gui::{GuiApp, emit};

pub(super) fn finish(
    app: &GuiApp,
    result: Result<serde_json::Value, BridgeError>,
    reply_to: ReplyId,
) {
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
