//! Personal provider keys (personal subscriptions such as `OpenCode` Go): list,
//! store and remove the signed-in user's keys through the gateway.
//!
//! A stored or removed key changes which models the gateway profile serves
//! this user, so a successful write re-runs sync, which rewrites every host
//! config (`OpenCode`'s `opencode.json` among them) from the fresh profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_models::bridge::profile::SetProviderCredentialRequest;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::gui::GuiApp;
use crate::gui::error::{GuiError, GuiResult};
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::finish;

#[derive(Clone)]
pub enum PersonalKeyOp {
    List,
    Set {
        provider: String,
        api_key: String,
        label: Option<String>,
    },
    Remove {
        provider: String,
    },
}

// Why: `HostUiEvent` is traced with `{:?}`; the key must never reach a log.
impl std::fmt::Debug for PersonalKeyOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::List => f.write_str("List"),
            Self::Set { provider, .. } => f
                .debug_struct("Set")
                .field("provider", provider)
                .finish_non_exhaustive(),
            Self::Remove { provider } => f
                .debug_struct("Remove")
                .field("provider", provider)
                .finish(),
        }
    }
}

pub(crate) fn on_personal_key_requested(app: &GuiApp, op: PersonalKeyOp, reply_to: ReplyId) {
    match &op {
        PersonalKeyOp::List => {},
        PersonalKeyOp::Set { provider, .. } => {
            app.append_log(format!(
                "personal key: verifying and storing for {provider}"
            ));
        },
        PersonalKeyOp::Remove { provider } => {
            app.append_log(format!("personal key: removing for {provider}"));
        },
    }
    let resync = !matches!(op, PersonalKeyOp::List);
    let proxy = app.proxy.clone();
    let http = app.ctx.http.clone();
    app.ctx.spawn(async move {
        let result = run(op, http).await.map_err(|e| to_bridge_error(&e));
        proxy.send_event(UiEvent::Host(HostUiEvent::PersonalKeyFinished {
            result,
            resync,
            reply_to,
        }));
    });
}

// JSON: webview IPC reply — the provider rows or the stored row.
async fn run(op: PersonalKeyOp, http: reqwest::Client) -> GuiResult<Value> {
    let cfg = config::load()?;
    let gateway_base = config::gateway_url_or_default(&cfg);
    let bearer = crate::auth::cache::read_for(&cfg, &gateway_base, 30)
        .map_err(|e| GuiError::Profile {
            context: "credential cache".into(),
            source: e,
        })?
        .ok_or(GuiError::NotAuthenticated)?;
    let client = GatewayClient::new(gateway_base, http);
    let token = &bearer.token;
    Ok(match op {
        PersonalKeyOp::List => json!(client.list_personal_keys(token).await?),
        PersonalKeyOp::Set {
            provider,
            api_key,
            label,
        } => {
            let body = SetProviderCredentialRequest { api_key, label };
            json!(client.set_personal_key(token, &provider, &body).await?)
        },
        PersonalKeyOp::Remove { provider } => {
            client.delete_personal_key(token, &provider).await?;
            json!({ "provider": provider })
        },
    })
}

// Why: the settings row picks its badge and fix button from `detail.reason`,
// so each gateway answer the user can act on maps to its own reason.
fn to_bridge_error(error: &GuiError) -> BridgeError {
    let (code, reason, message) = match error {
        GuiError::NotAuthenticated => (
            ErrorCode::Unauthorized,
            "signed_out",
            "Sign in to manage personal keys".to_owned(),
        ),
        GuiError::Gateway(GatewayError::Rejected {
            status, rejection, ..
        }) => {
            let message = rejection
                .message
                .clone()
                .unwrap_or_else(|| format!("gateway answered {status}"));
            match status.as_u16() {
                401 => (ErrorCode::Unauthorized, "signed_out", message),
                404 => (ErrorCode::NotFound, "not_offered", message),
                422 => (ErrorCode::InvalidArgs, "key_rejected", message),
                400 => (ErrorCode::Unreachable, "unverified", message),
                _ => (ErrorCode::Internal, "gateway_error", message),
            }
        },
        GuiError::Gateway(other) => (ErrorCode::Unreachable, "unreachable", other.to_string()),
        other => (ErrorCode::Internal, "gateway_error", other.to_string()),
    };
    BridgeError::new(ErrorScope::Gateway, code, message).with_detail(json!({ "reason": reason }))
}

pub(crate) fn on_personal_key_finished(
    app: &GuiApp,
    // JSON: webview IPC reply — the provider rows or the stored row.
    result: Result<Value, BridgeError>,
    resync: bool,
    reply_to: ReplyId,
) {
    match &result {
        Ok(_) if resync => {
            app.append_log("personal keys changed; re-syncing hosts with the new model list");
            app.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
        },
        Ok(_) => {},
        Err(e) => app.append_log_error(format!("personal key: {}", e.message)),
    }
    finish(app, result, reply_to);
}
