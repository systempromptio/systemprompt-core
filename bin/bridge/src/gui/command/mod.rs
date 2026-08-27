//! GUI command argument types and their handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod args;
mod general;
mod hosts;

use serde_json::Value;

use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

use general::{
    auth_dispatch, diagnostics_dispatch, gateway_dispatch, meta_dispatch, sync_dispatch,
};
use hosts::{agent_dispatch, host_dispatch};

#[derive(Debug)]
pub enum CommandOutcome {
    Sync(Result<Value, BridgeError>),
    Async,
}

pub(crate) fn dispatch(app: &GuiApp, id: u64, cmd: &str, args: &Value) -> CommandOutcome {
    let reply_id: ReplyId = Some(id);
    if let Some(out) = meta_dispatch(app, cmd, args, reply_id) {
        return out;
    }
    if let Some(out) = gateway_dispatch(app, cmd, args.clone(), reply_id) {
        return out;
    }
    if let Some(out) = auth_dispatch(app, cmd, args.clone(), reply_id) {
        return out;
    }
    if let Some(out) = sync_dispatch(app, cmd, args.clone(), reply_id) {
        return out;
    }
    if let Some(out) = host_dispatch(app, cmd, args.clone(), reply_id) {
        return out;
    }
    if let Some(out) = agent_dispatch(app, cmd, args.clone(), reply_id) {
        return out;
    }
    if let Some(out) = diagnostics_dispatch(app, cmd, reply_id) {
        return out;
    }
    CommandOutcome::Sync(Err(BridgeError::new(
        ErrorScope::Internal,
        ErrorCode::NotFound,
        format!("unknown command: {cmd}"),
    )))
}

pub(super) fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, BridgeError> {
    serde_json::from_value(args).map_err(|e| BridgeError::invalid_args(e.to_string()))
}

pub(super) fn send(app: &GuiApp, event: UiEvent) {
    app.proxy.send_event(event);
}

pub(crate) fn reply_for_value(result: Result<Value, BridgeError>) -> IpcReplyPayload {
    match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(e) => IpcReplyPayload::err(e),
    }
}
