//! Steps an install leaves for the user — approving a macOS profile,
//! restarting an open app — recorded as host state, plus the restart that
//! clears the second one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use serde_json::json;
use systemprompt_models::bridge::host::HostKind;

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::integration::agent_health::PendingStep;
use crate::integration::{ConfigFormat, find_host_by_id};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::finish;

// Why: `open -a` on an app that is still quitting re-focuses the dying
// instance instead of launching a fresh one.
const QUIT_SETTLE: Duration = Duration::from_secs(2);

pub(crate) fn note_install_pending(app: &GuiApp, host_id: HostKind) {
    let Some(host) = find_host_by_id(host_id) else {
        return;
    };
    // Why: macOS installs a configuration profile only after the user approves
    // it in System Settings; until then the probe reads no profile at all.
    let held_by_os = cfg!(target_os = "macos") && host.config_format() == ConfigFormat::Plist;
    let running = app
        .state
        .snapshot()
        .hosts
        .get(host_id)
        .and_then(|s| s.snapshot.as_ref())
        .is_some_and(|s| s.host_running == Some(true));
    let step = if held_by_os {
        PendingStep::OsApproval {
            offered_at_unix: crate::gui::state::now_unix(),
        }
    } else if running {
        PendingStep::Restart
    } else {
        PendingStep::None
    };
    app.state.set_host_pending(host_id, step);
}

pub(crate) fn on_restart_requested(app: &GuiApp, host_id: HostKind, reply_to: ReplyId) {
    let Some(host) =
        crate::gui::hosts::resolve::resolve_or_reply(app, host_id, "restart", reply_to)
    else {
        return;
    };
    if let Err(err) = host.quit() {
        let msg = format!(
            "could not quit {}: {err} — quit it yourself and open it again",
            host.display_name()
        );
        app.append_log_error(&msg);
        finish(
            app,
            Err(BridgeError::new(ErrorScope::Host, ErrorCode::Internal, msg)),
            reply_to,
        );
        return;
    }
    app.append_log(format!(
        "[{host_id}] quit {} to restart it",
        host.display_name()
    ));
    let proxy = app.proxy.clone();
    app.ctx.spawn(async move {
        tokio::time::sleep(QUIT_SETTLE).await;
        match tokio::task::spawn_blocking(move || host.open()).await {
            Ok(Ok(())) => {},
            Ok(Err(err)) => tracing::warn!(host_id = %host_id, error = %err, "relaunch failed"),
            Err(err) => tracing::warn!(host_id = %host_id, error = %err, "relaunch task failed"),
        }
        proxy.send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
            host_id,
            cause: ProbeCause::Manual,
            reply_to: None,
        }));
    });
    finish(app, Ok(json!({ "restarted": true })), reply_to);
}
