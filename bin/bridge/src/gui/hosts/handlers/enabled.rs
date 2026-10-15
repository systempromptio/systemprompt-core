//! Per-host enable-preference update handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;
use systemprompt_models::bridge::host::HostKind;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::GuiApp;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::finish;

pub(crate) fn on_enabled_set_requested(
    app: &GuiApp,
    host_id: HostKind,
    enabled: bool,
    reply_to: ReplyId,
) {
    app.append_log(format!(
        "[{host_id}] {} for this account",
        if enabled { "turning on" } else { "turning off" }
    ));
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = push_enabled(host_id, enabled, http).await.map_err(Arc::new);
        proxy.send_event(UiEvent::Host(HostUiEvent::EnabledSetFinished {
            host_id,
            enabled,
            result,
            reply_to,
        }));
    });
}

async fn push_enabled(host_id: HostKind, enabled: bool, http: reqwest::Client) -> GuiResult<()> {
    let cfg = config::load()?;
    let gateway_base = config::gateway_url_or_default(&cfg);
    let bearer = crate::auth::cache::read_for(&cfg, &gateway_base, 30)
        .map_err(|e| GuiError::Profile {
            context: "credential cache".into(),
            source: e,
        })?
        .ok_or(GuiError::NotAuthenticated)?;
    GatewayClient::new(gateway_base, http)
        .set_host_enabled(&bearer.token, host_id, enabled)
        .await?;
    Ok(())
}

pub(crate) fn on_enabled_set_finished(
    app: &GuiApp,
    host_id: HostKind,
    enabled: bool,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(format!("[{host_id}] enable preference saved; re-syncing"));
            app.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            Ok(json!({ "host_id": host_id, "enabled": enabled }))
        },
        Err(e) => {
            let line = format!("[{host_id}] enable preference update failed: {e}");
            app.append_log_error(&line);
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    finish(app, bridge_result, reply_to);
}
