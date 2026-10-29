//! Gateway profile, instance and MCP auth-probe command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::config::gateways::GatewayProfileId;
use crate::gui::GuiApp;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::handlers::instance::InstanceOp;
use crate::ids::McpServerId;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::args::{GatewayProfileArgs, GatewaySetArgs, McpProbeArgs};
use super::{CommandOutcome, parse, send};

// JSON: webview IPC args — decoded per command with `parse::<T>`.
pub(super) fn gateway_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "gateway.set" => match parse::<GatewaySetArgs>(args) {
            Ok(a) if a.url.trim().is_empty() => CommandOutcome::Sync(Err(BridgeError::new(
                ErrorScope::Gateway,
                ErrorCode::InvalidArgs,
                "gateway url is empty",
            ))),
            Ok(a) => {
                send(
                    app,
                    UiEvent::SetGatewayRequested {
                        url: a.url,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "gateway.probe" => {
            send(app, UiEvent::GatewayProbeRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "gateway.list" => CommandOutcome::Sync(
            crate::config::gateways::list()
                .map(|profiles| json!({ "profiles": profiles }))
                .map_err(|e| BridgeError::from_error(ErrorScope::Gateway, ErrorCode::Internal, &e)),
        ),
        "gateway.add" => match parse::<GatewaySetArgs>(args) {
            Ok(a) if a.url.trim().is_empty() => CommandOutcome::Sync(Err(BridgeError::new(
                ErrorScope::Gateway,
                ErrorCode::InvalidArgs,
                "gateway url is empty",
            ))),
            Ok(a) => instance(app, InstanceOp::Add(a.url.trim().to_owned()), reply_id),
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "gateway.use" | "gateway.remove" => match parse::<GatewayProfileArgs>(args).and_then(|a| {
            GatewayProfileId::try_new(a.id).map_err(|e| {
                BridgeError::from_error(ErrorScope::Gateway, ErrorCode::InvalidArgs, &e)
            })
        }) {
            Ok(id) if cmd == "gateway.use" => instance(app, InstanceOp::Use(id), reply_id),
            Ok(id) => instance(app, InstanceOp::Remove(id), reply_id),
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "elevation.approve" => {
            app.ctx.elevation_gate.reset_decline();
            app.state.reload();
            CommandOutcome::Sync(Ok(json!({})))
        },
        "instance.disable" => instance(app, InstanceOp::Disable, reply_id),
        "instance.enable" => instance(app, InstanceOp::Enable, reply_id),
        "mcp.auth.probe" => {
            let server_id = parse::<McpProbeArgs>(args)
                .inspect_err(|e| tracing::warn!(error = ?e, "malformed mcp.auth.probe args"))
                .ok()
                .and_then(|a| a.server_id)
                .and_then(|s| McpServerId::try_new(s).ok());
            send(
                app,
                UiEvent::McpAuthProbeRequested {
                    server_id,
                    reply_to: reply_id,
                },
            );
            CommandOutcome::Async
        },
        _ => return None,
    })
}

fn instance(app: &GuiApp, op: InstanceOp, reply_id: ReplyId) -> CommandOutcome {
    send(
        app,
        UiEvent::InstanceRequested {
            op,
            reply_to: reply_id,
        },
    );
    CommandOutcome::Async
}
