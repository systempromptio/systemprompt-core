//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::{GuiApp, emit};
use crate::proxy::mcp_probe::{self, McpServerAuth};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_mcp_auth_probe_requested(app: &mut GuiApp, reply_to: ReplyId) {
    if !app.state.mark_mcp_auth_probing() {
        if let Some(id) = reply_to {
            emit::send_reply(app, id, json!({ "inFlight": true }), true);
        }
        return;
    }
    app.refresh_ui();
    emit::emit_mcp_changed(app);
    spawn_probe(app, reply_to);
}

fn spawn_probe(app: &GuiApp, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let results = mcp_probe::probe_all().await;
        _ = proxy.send_event(UiEvent::McpAuthProbeFinished { results, reply_to });
    });
}

pub(crate) fn on_mcp_auth_probe_finished(
    app: &mut GuiApp,
    results: Vec<McpServerAuth>,
    reply_to: ReplyId,
) {
    app.state.apply_mcp_auth(results);
    app.refresh_ui();
    emit::emit_mcp_changed(app);
    emit::emit_state(app);
    if let Some(id) = reply_to {
        emit::send_reply(app, id, json!({}), true);
    }
}
