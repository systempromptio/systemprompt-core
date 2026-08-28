//! GUI handlers probing MCP server auth reachability.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::notify::Signal;
use crate::gui::{GuiApp, emit};
use crate::proxy::mcp_probe::{self, McpAuthState, McpServerAuth};

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
        proxy.send_event(UiEvent::McpAuthProbeFinished { results, reply_to });
    });
}

pub(crate) fn on_mcp_auth_probe_finished(
    app: &mut GuiApp,
    results: Vec<McpServerAuth>,
    reply_to: ReplyId,
) {
    let broken: Vec<String> = results
        .iter()
        .filter(|r| {
            matches!(
                r.state,
                McpAuthState::GatewayUnauthorized | McpAuthState::NotRegistered
            )
        })
        .map(|r| r.id.clone())
        .collect();
    app.state.apply_mcp_auth(results);
    if broken.is_empty() {
        app.signal_cleared(Signal::McpAuthBroken);
    } else {
        app.signal_raised(
            Signal::McpAuthBroken,
            &format!(
                "{} cannot authenticate to MCP",
                crate::brand::brand().app_name
            ),
            &format!("Affected servers: {}", broken.join(", ")),
        );
    }
    app.refresh_ui();
    emit::emit_mcp_changed(app);
    emit::emit_state(app);
    if let Some(id) = reply_to {
        emit::send_reply(app, id, json!({}), true);
    }
}
