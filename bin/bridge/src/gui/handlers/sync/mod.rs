//! GUI handlers driving manifest sync and reporting progress.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod finished;

pub(crate) use finished::on_sync_finished;

use std::sync::Arc;

use crate::gateway::Freshness;
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::{config, sync};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_sync_requested(app: &mut GuiApp, reply_to: ReplyId) {
    if app.state.snapshot().sync_in_flight {
        // Why: a request with no reply channel comes from login or a gateway
        // change, and dropping it left the new gateway unsynced behind a run
        // for the old one. It waits for that run to finish instead.
        match reply_to {
            Some(id) => {
                let err = BridgeError::new(
                    ErrorScope::Marketplace,
                    ErrorCode::Conflict,
                    "sync already in flight",
                );
                emit::send_reply_payload(app, id, &IpcReplyPayload::err(err));
            },
            None => app.state.set_sync_pending(true),
        }
        return;
    }
    app.did_initial_sync = true;
    app.state.set_sync_in_flight(true);
    app.append_log("Sync started…");
    app.refresh_ui();
    emit::emit_sync_progress(app, "started", None);
    let proxy = app.proxy.clone();
    let token = app.state.install_cancel(CancelScope::Sync);
    let bridge = Arc::clone(&app.ctx);
    {
        let proxy = proxy.clone();
        bridge.sync_progress.install(Arc::new(move |step| {
            proxy.send_event(UiEvent::SyncStep(step.clone()));
        }));
    }
    app.ctx.spawn(async move {
        let allow_tofu = matches!(
            config::pinned_pubkey_state(),
            Ok(config::PinnedPubkeyState::Unpinned)
        );
        // Why: a sync the user pressed usually follows something they just
        // did on the gateway (linking a connector); the memo cannot see it.
        let freshness = if reply_to.is_some() {
            Freshness::Fresh
        } else {
            Freshness::Memo
        };
        let options = sync::SyncOptions {
            allow_unsigned: false,
            force_replay: false,
            allow_tofu,
            freshness,
            cancel: token,
        };
        let result = sync::run_once(&bridge, &options)
            .await
            .map_err(|e| match e {
                sync::SyncError::Cancelled { .. } => GuiError::Cancelled,
                other => GuiError::from(other),
            })
            .map_err(Arc::new);
        bridge.sync_progress.clear();
        proxy.send_event(UiEvent::SyncFinished { result, reply_to });
    });
}

pub(crate) fn on_sync_started(app: &mut GuiApp) {
    app.state.set_sync_in_flight(true);
    app.refresh_ui();
}
