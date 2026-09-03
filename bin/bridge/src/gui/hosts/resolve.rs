//! One answer to "what may this binary do with this host id".
//!
//! Three of the four [`ResolvedHost`] states are ordinary and must not reach a
//! user as an error. A sync-only agent (`claude-code`) is governed from the
//! gateway and installs nothing here; a suppressed host is one a white-label
//! build deliberately does not offer; only an id belonging to neither the
//! registry nor `KNOWN_HOSTS` is a caller error.
//!
//! Every per-host handler used to re-derive that itself, and v0.43.0 shipped
//! with all seven of them answering `unknown host: claude-code` — a toast
//! naming the agent most readers are running. This is the single place that
//! decision is made, so the eighth handler cannot regress it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use crate::gui::GuiApp;
use crate::gui::events::ReplyId;
use crate::integration::host_app::HostApp;
use crate::integration::{ResolvedHost, resolve_host};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::handlers::finish;

pub(crate) fn resolve_or_reply(
    app: &GuiApp,
    host_id: &str,
    action: &str,
    reply_to: ReplyId,
) -> Option<&'static dyn HostApp> {
    match resolve_host(host_id) {
        ResolvedHost::Local(host) => Some(host),
        ResolvedHost::SyncOnly(agent) => {
            // Why: the activity log is where a user goes to find out why
            // nothing happened. Silence here reads as a dropped click.
            app.append_log(format!(
                "[{host_id}] {action}: nothing to do — {} is governed through the gateway and \
                 installs nothing on this computer",
                agent.display_name
            ));
            finish(
                app,
                Ok(json!({
                    "host_id": agent.id,
                    "changed": false,
                    "reason": "sync-only",
                    "detail": agent.description,
                })),
                reply_to,
            );
            None
        },
        ResolvedHost::Suppressed => {
            app.append_log(format!(
                "[{host_id}] {action}: nothing to do — this agent is not offered on this \
                 installation"
            ));
            finish(
                app,
                Ok(json!({
                    "host_id": host_id,
                    "changed": false,
                    "reason": "not-offered",
                })),
                reply_to,
            );
            None
        },
        ResolvedHost::Unknown => {
            app.append_log_error(format!("{action}: unknown host '{host_id}'"));
            finish(
                app,
                Err(BridgeError::new(
                    ErrorScope::Host,
                    ErrorCode::NotFound,
                    format!("unknown host: {host_id}"),
                )),
                reply_to,
            );
            None
        },
    }
}
