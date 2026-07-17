//! GUI handler cancelling in-flight operations by scope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::ReplyId;
use crate::gui::ipc::IpcReplyPayload;
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_cancel_in_flight(app: &GuiApp, scope: Option<CancelScope>, reply_to: ReplyId) {
    let cancelled = scope.map_or_else(
        || {
            app.state.cancel_all();
            app.append_log("Cancelled in-flight tasks");
            true
        },
        |s| {
            let fired = app.state.cancel_scope(s);
            if fired {
                app.append_log(format!("Cancelled {}", scope_label(s)));
            }
            fired
        },
    );
    if let Some(id) = reply_to {
        emit::send_reply_payload(
            app,
            id,
            &IpcReplyPayload::ok(json!({ "cancelled": cancelled })),
        );
    }
}

const fn scope_label(scope: CancelScope) -> &'static str {
    match scope {
        CancelScope::Sync => "sync",
        CancelScope::Login => "login",
        CancelScope::GatewayProbe => "gateway probe",
    }
}
