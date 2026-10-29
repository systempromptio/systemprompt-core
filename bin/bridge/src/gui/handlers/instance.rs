//! GUI handlers for gateway profiles and the user's opt-in disable.
//!
//! Every operation runs off the event loop and reports back through
//! [`UiEvent::InstanceFinished`]. A switch or an enable ends with a sync, as a
//! login does; a disable ends with the host list redrawn from local state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::config::gateways::{self, GatewayProfileId};
use crate::gui::error::GuiError;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, emit};
use crate::integration::lifecycle;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

#[derive(Debug, Clone)]
pub enum InstanceOp {
    Add(String),
    Use(GatewayProfileId),
    Remove(GatewayProfileId),
    Disable,
    Enable,
}

impl InstanceOp {
    const fn label(&self) -> &'static str {
        match self {
            Self::Add(_) => "gateway.add",
            Self::Use(_) => "gateway.use",
            Self::Remove(_) => "gateway.remove",
            Self::Disable => "instance.disable",
            Self::Enable => "instance.enable",
        }
    }

    const fn syncs_after(&self) -> bool {
        matches!(self, Self::Use(_))
    }
}

pub(crate) fn on_instance_requested(app: &GuiApp, op: InstanceOp, reply_to: ReplyId) {
    if matches!(op, InstanceOp::Use(_) | InstanceOp::Disable) {
        app.state.cancel_scope(CancelScope::Sync);
    }
    app.append_log(format!("{}: running", op.label()));
    let proxy = app.proxy.clone();
    let ctx = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let worker = Arc::clone(&ctx);
        let blocking_op = op.clone();
        let result = match tokio::task::spawn_blocking(move || run(&worker, &blocking_op)).await {
            Ok(result) => result,
            Err(join_err) => Err(Arc::new(GuiError::Io(std::io::Error::other(join_err)))),
        };
        if let Ok(value) = &result {
            refresh_discovery(&ctx, &op, value).await;
        }
        proxy.send_event(UiEvent::InstanceFinished {
            op,
            result,
            reply_to,
        });
    });
}

// JSON: webview IPC reply — the instance operation's outcome as the GUI reads
// it
fn run(ctx: &crate::context::BridgeContext, op: &InstanceOp) -> Result<Value, Arc<GuiError>> {
    let wrap = Arc::new;
    match op {
        InstanceOp::Add(url) => gateways::add(url)
            .map(|id| json!({ "id": id, "machine_profile": machine_profile(ctx) }))
            .map_err(|e| wrap(e.into())),
        InstanceOp::Use(id) => lifecycle::switch(ctx, id)
            .map(|report| {
                json!({
                    "id": report.profile.id,
                    "foreign_plugins": report.released.foreign_plugins,
                    "warnings": report.released.warnings,
                })
            })
            .map_err(|e| wrap(e.into())),
        InstanceOp::Remove(id) => gateways::remove(id)
            .map(|()| json!({ "machine_profile": machine_profile(ctx) }))
            .map_err(|e| wrap(e.into())),
        InstanceOp::Disable => lifecycle::disable(ctx)
            .map(|report| {
                json!({
                    "already_disabled": report.already_disabled,
                    "still_managed": report.still_managed,
                    "foreign_plugins": report.released.foreign_plugins,
                    "warnings": report.released.warnings,
                })
            })
            .map_err(|e| wrap(e.into())),
        InstanceOp::Enable => {
            let changed = lifecycle::enable(ctx).map_err(|e| wrap(e.into()))?;
            if !changed {
                return Ok(json!({ "changed": false }));
            }
            let options = crate::sync::SyncOptions {
                allow_unsigned: false,
                force_replay: false,
                allow_tofu: false,
                freshness: crate::gateway::Freshness::Fresh,
                cancel: tokio_util::sync::CancellationToken::new(),
            };
            let synced = ctx.block_on(crate::sync::run_once(ctx, &options));
            let (sync_line, restored) = match synced {
                Ok(summary) => {
                    let restored = ctx
                        .block_on(lifecycle::re_enrol_delivered_hosts(ctx))
                        .map_err(|e| wrap(GuiError::Io(std::io::Error::other(e))))?;
                    (
                        summary.one_line(),
                        crate::integration::enrol::render(&restored),
                    )
                },
                Err(e) => return Err(wrap(e.into())),
            };
            Ok(json!({ "changed": true, "sync": sync_line, "hosts": restored }))
        },
    }
}

// JSON: webview IPC reply — the instance operation's outcome as the GUI reads
// it
fn machine_profile(ctx: &crate::context::BridgeContext) -> Value {
    use crate::integration::lifecycle::MachineProfileRefresh;
    match ctx.block_on(lifecycle::refresh_machine_profile(ctx)) {
        MachineProfileRefresh::Current => json!({ "state": "current" }),
        MachineProfileRefresh::Rewritten => json!({ "state": "rewritten" }),
        MachineProfileRefresh::AwaitingApproval => json!({ "state": "awaiting-approval" }),
        MachineProfileRefresh::ManagedElsewhere { missing } => {
            json!({ "state": "managed-elsewhere", "missing": missing })
        },
    }
}

// JSON: webview IPC reply — the instance operation's outcome as the GUI reads
// it
async fn refresh_discovery(ctx: &crate::context::BridgeContext, op: &InstanceOp, value: &Value) {
    let result = match op {
        InstanceOp::Add(_) => {
            let Some(id) = value
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| GatewayProfileId::try_new(id).ok())
            else {
                return;
            };
            match gateways::profile(&id) {
                Ok(profile) => crate::gateway::discovery::refresh_profile(&ctx.http, &profile)
                    .await
                    .map(|_| ()),
                Err(e) => Err(e.into()),
            }
        },
        InstanceOp::Use(_) => crate::gateway::discovery::refresh_active(&ctx.http)
            .await
            .map(|_| ()),
        InstanceOp::Remove(_) | InstanceOp::Disable | InstanceOp::Enable => return,
    };
    if let Err(error) = result {
        tracing::warn!(%error, "instance discovery unavailable; using built-in routes");
    }
}

pub(crate) fn on_instance_finished(
    app: &mut GuiApp,
    op: &InstanceOp,
    // JSON: webview IPC reply — the instance operation's outcome as the GUI reads it
    result: Result<Value, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let reply = match result {
        Ok(value) => {
            app.append_log(format!("{}: done", op.label()));
            for warning in value
                .get("warnings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                app.append_log_error(format!("{}: {warning}", op.label()));
            }
            Ok(value)
        },
        Err(e) => {
            let line = format!("{}: {e}", op.label());
            app.append_log_error(&line);
            Err(BridgeError::new(
                ErrorScope::Gateway,
                ErrorCode::Internal,
                line,
            ))
        },
    };
    let succeeded = reply.is_ok();
    app.state.reload();
    app.refresh_ui();
    emit::emit_gateway_changed(app);
    if succeeded && op.syncs_after() {
        app.proxy
            .send_event(UiEvent::SyncRequested { reply_to: None });
        crate::gui::handlers::gateway_probe::spawn_probe(app, None);
    }
    emit::finish(app, reply_to, reply);
}

pub(crate) fn spawn_discovery_refresh(app: &GuiApp) {
    let ctx = Arc::clone(&app.ctx);
    let proxy = app.proxy.clone();
    app.ctx.spawn(async move {
        if let Err(error) = crate::gateway::discovery::refresh_active(&ctx.http).await {
            tracing::warn!(%error, "instance discovery unavailable; using built-in routes");
        }
        proxy.send_event(UiEvent::StateRefreshed);
    });
}
