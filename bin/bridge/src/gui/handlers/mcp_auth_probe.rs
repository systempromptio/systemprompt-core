//! GUI handlers probing MCP server auth reachability.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::events::{McpProbeResults, ReplyId, UiEvent};
use crate::gui::notify::Signal;
use crate::gui::{GuiApp, emit};
use crate::proxy::mcp_probe;

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_mcp_auth_probe_requested(
    app: &mut GuiApp,
    server_id: Option<String>,
    reply_to: ReplyId,
) {
    if !app.state.mark_mcp_auth_probing() {
        if let Some(id) = reply_to {
            emit::send_reply(app, id, json!({ "inFlight": true }), true);
        }
        return;
    }
    app.refresh_ui();
    emit::emit_mcp_changed(app);
    spawn_probe(app, server_id, reply_to);
}

fn spawn_probe(app: &GuiApp, server_id: Option<String>, reply_to: ReplyId) {
    let proxy = app.proxy.clone();
    let loopback = app.ctx.proxy.loopback().clone();
    let registry = app.ctx.mcp_registry();
    app.ctx.spawn(async move {
        let results = match server_id {
            Some(slug) => {
                McpProbeResults::One(mcp_probe::probe_slug(&loopback, &registry, &slug).await)
            },
            None => McpProbeResults::All(mcp_probe::probe_all(&loopback, &registry).await),
        };
        proxy.send_event(UiEvent::McpAuthProbeFinished { results, reply_to });
    });
}

pub(crate) fn on_mcp_auth_probe_finished(
    app: &mut GuiApp,
    results: McpProbeResults,
    reply_to: ReplyId,
) {
    match results {
        McpProbeResults::All(results) => {
            remember_tools(app, &results);
            app.state.apply_mcp_auth(results);
        },
        McpProbeResults::One(Some(result)) => {
            remember_tools(app, std::slice::from_ref(&result));
            app.state.apply_mcp_auth_one(result);
        },
        McpProbeResults::One(None) => app.state.apply_mcp_auth(Vec::new()),
    }
    let broken: Vec<String> = app
        .state
        .snapshot()
        .mcp_auth
        .iter()
        .filter(|r| r.state.needs_sign_in())
        .map(|r| r.id.clone())
        .collect();
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

// Why: the desktop tool policy is written from this same tool list; every
// probe that reaches a server keeps the catalog current between syncs.
fn remember_tools(app: &GuiApp, results: &[mcp_probe::McpServerAuth]) {
    if let Err(e) = crate::install::mdm::tool_catalog::record(results) {
        tracing::warn!(error = %e, "mcp tool catalog not updated from probe");
        app.append_log_warn(format!(
            "tool catalog not updated from the probe ({e}); the desktop tool policy keeps its \
             last names"
        ));
    }
}
