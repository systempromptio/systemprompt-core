use serde_json::json;

use crate::gui::GuiApp;
use crate::gui::events::ReplyId;
use crate::gui::ipc::IpcReplyPayload;
use crate::gui::ipc_runtime;
use crate::gui::state::CancelScope;

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_cancel_in_flight(app: &mut GuiApp, scope: Option<CancelScope>, reply_to: ReplyId) {
    let cancelled = match scope {
        Some(s) => {
            let fired = app.state.cancel_scope(s);
            if fired {
                app.append_log(format!("Cancelled {}", scope_label(s)));
            }
            fired
        },
        None => {
            app.state.cancel_all();
            app.append_log("Cancelled in-flight tasks");
            true
        },
    };
    if let Some(id) = reply_to {
        ipc_runtime::send_reply_payload(
            app,
            id,
            &IpcReplyPayload::ok(json!({ "cancelled": cancelled })),
        );
    }
}

fn scope_label(scope: CancelScope) -> &'static str {
    match scope {
        CancelScope::Sync => "sync",
        CancelScope::Login => "login",
        CancelScope::GatewayProbe => "gateway probe",
    }
}
