//! Per-host model-filter update handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gui::GuiApp;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope};
use crate::ids::HostId;
use crate::integration::find_host_by_id;

use super::finish;

pub(crate) fn on_model_filter_set_requested(
    app: &GuiApp,
    host_id: &HostId,
    protocols: Option<Vec<String>>,
    reply_to: ReplyId,
) {
    if find_host_by_id(host_id.as_str()).is_none() {
        let err = BridgeError::new(
            ErrorScope::Host,
            ErrorCode::NotFound,
            format!("unknown host: {host_id}"),
        );
        finish(app, Err(err), reply_to);
        return;
    }
    match &protocols {
        Some(list) if list.is_empty() => {
            app.append_log(format!("[{host_id}] model filter → all models"));
        },
        Some(list) => {
            app.append_log(format!("[{host_id}] model filter → {}", list.join(", ")));
        },
        None => app.append_log(format!("[{host_id}] model filter cleared (host default)")),
    }
    let host_id_owned = host_id.clone();
    let proxy = app.proxy.clone();
    app.runtime.spawn(async move {
        let result = push_model_filter(&host_id_owned, protocols.as_deref())
            .await
            .map_err(Arc::new);
        proxy.send_event(UiEvent::Host(HostUiEvent::ModelFilterSetFinished {
            host_id: host_id_owned,
            result,
            reply_to,
        }));
    });
}

async fn push_model_filter(host_id: &HostId, protocols: Option<&[String]>) -> GuiResult<()> {
    let cfg = config::load();
    let gateway_base = config::gateway_url_or_default(&cfg);
    let bearer =
        crate::auth::cache::read_valid(&gateway_base).ok_or_else(|| GuiError::Profile {
            context: "model filter".into(),
            source: std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "not signed in; cannot update host model filter",
            ),
        })?;
    GatewayClient::new(gateway_base)
        .set_host_model_filter(bearer.token.expose(), host_id.as_str(), protocols)
        .await
        .map_err(|e| GuiError::Profile {
            context: "host model filter".into(),
            source: std::io::Error::other(e.to_string()),
        })
}

pub(crate) fn on_model_filter_set_finished(
    app: &GuiApp,
    host_id: &HostId,
    result: Result<(), Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let bridge_result = match result {
        Ok(()) => {
            app.append_log(format!("[{host_id}] model filter saved; re-syncing"));
            app.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
            Ok(json!({ "host_id": host_id }))
        },
        Err(e) => {
            let line = format!("[{host_id}] model filter update failed: {e}");
            app.append_log(&line);
            Err(BridgeError::new(
                ErrorScope::Host,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    finish(app, bridge_result, reply_to);
}
