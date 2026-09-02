//! No-op replies for agents governed from the gateway.
//!
//! A sync-only agent (`integration::SYNC_ONLY_AGENTS` — `claude-code`) appears
//! in the Agents list like any other, but implements no [`HostApp`], so
//! `find_host_by_id` cannot see it. Every per-host handler used to answer the
//! same way: `BridgeError(Host, NotFound, "unknown host: claude-code")`, shown
//! to the user as a toast and written to the activity log on every "Re-check
//! all". The id was never unknown — it was the one most readers are running.
//!
//! `HostEntryPayload`'s `can_*` flags now keep those affordances off the
//! screen, so this is the second line: a stale front end, an in-flight click
//! from before a sync, or a future caller must still get a truthful answer
//! rather than an error naming a host that exists. Nothing is installed
//! locally, so the honest answer to "install / repair / probe / remove it" is
//! that there was nothing to do — not that the agent is unknown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::GuiApp;

// Why: `None` means the id is not sync-only, so the caller's own not-found
// path applies rather than this shim answering for it.
pub(crate) fn noop_reply(app: &GuiApp, host_id: &str, action: &str) -> Option<serde_json::Value> {
    let agent = crate::integration::sync_only_agent(host_id)?;
    // Why: the activity log is where a user goes to find out why nothing
    // happened. Silence here reads as a dropped click.
    app.append_log(format!(
        "[{host_id}] {action}: nothing to do — {} is governed through the gateway and installs \
         nothing on this computer",
        agent.display_name
    ));
    Some(json!({
        "host_id": agent.id,
        "changed": false,
        "reason": "sync-only",
        "detail": agent.description,
    }))
}
