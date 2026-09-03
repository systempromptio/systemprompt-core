//! Meta, gateway, auth and sync command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, server_json};
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope};

use super::args::{
    CancelArgs, GatewaySetArgs, LoginArgs, McpProbeArgs, OpenExternalUrlArgs, RecentArgs,
    SessionLoginArgs,
};
use super::{CommandOutcome, parse, send};

fn is_safe_external_url(url: &str) -> bool {
    url.starts_with("https://")
}

const DEFAULT_RECENT_LIMIT: usize = 500;
const MAX_RECENT_LIMIT: usize = 2000;

fn recent_limit(args: &Value) -> usize {
    parse::<RecentArgs>(args.clone())
        .ok()
        .and_then(|a| a.limit)
        .unwrap_or(DEFAULT_RECENT_LIMIT)
        .min(MAX_RECENT_LIMIT)
}

pub(super) fn meta_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: &Value,
    _reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "state.snapshot" => CommandOutcome::Sync(Ok(server_json::snapshot_value(
            &app.state.snapshot(),
            &app.ctx.proxy,
        ))),
        "marketplace.list" => CommandOutcome::Sync(Ok(marketplace_listing(app))),
        "activity.recent" => CommandOutcome::Sync(Ok(json!({
            "entries": app.ctx.activity.snapshot_recent(recent_limit(args)),
        }))),
        "setup.complete" => {
            send(app, UiEvent::SetupComplete);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openConfigFolder" => {
            send(app, UiEvent::OpenConfigFolder);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openExternalUrl" => open_external_url(args.clone()),
        "quit" => {
            send(app, UiEvent::Quit);
            CommandOutcome::Sync(Ok(json!({})))
        },
        _ => return None,
    })
}

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
        "mcp.auth.probe" => {
            let server_id = parse::<McpProbeArgs>(args)
                .inspect_err(|e| tracing::warn!(error = ?e, "malformed mcp.auth.probe args"))
                .ok()
                .and_then(|a| a.server_id)
                .filter(|s| !s.is_empty());
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

pub(super) fn auth_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "login" => match parse::<LoginArgs>(args) {
            Ok(a) if a.token.expose().trim().is_empty() => CommandOutcome::Sync(Err(
                BridgeError::new(ErrorScope::Identity, ErrorCode::InvalidArgs, "PAT is empty"),
            )),
            Ok(a) => {
                send(
                    app,
                    UiEvent::LoginRequested {
                        token: a.token,
                        gateway: a.gateway,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "session.login" => match parse::<SessionLoginArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::SessionLoginRequested {
                        gateway: a.gateway,
                        keep_signed_in: a.keep_signed_in,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "logout" => {
            send(app, UiEvent::LogoutRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "profile.fetch" => {
            send(app, UiEvent::ProfileFetchRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        _ => return None,
    })
}

pub(super) fn sync_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "sync" => {
            send(app, UiEvent::SyncRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "validate" => {
            send(app, UiEvent::ValidateRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.check" => {
            send(app, UiEvent::UpdateCheckRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.install" => {
            send(app, UiEvent::UpdateInstallRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "update.restart" => {
            send(app, UiEvent::UpdateRestartRequested);
            CommandOutcome::Sync(Ok(Value::Null))
        },
        "settings.get" => {
            send(app, UiEvent::SettingsReadRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "cancel" => match parse::<CancelArgs>(args) {
            Ok(a) => match cancel_scope(a.scope.as_deref()) {
                Ok(scope) => {
                    send(
                        app,
                        UiEvent::CancelInFlight {
                            scope,
                            reply_to: reply_id,
                        },
                    );
                    CommandOutcome::Async
                },
                Err(e) => CommandOutcome::Sync(Err(e)),
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        _ => return None,
    })
}

fn cancel_scope(label: Option<&str>) -> Result<Option<CancelScope>, BridgeError> {
    Ok(match label {
        None | Some("all") => None,
        Some("sync") => Some(CancelScope::Sync),
        Some("login") => Some(CancelScope::Login),
        Some("gateway" | "gateway-probe") => Some(CancelScope::GatewayProbe),
        Some(other) => {
            return Err(BridgeError::invalid_args(format!(
                "unknown cancel scope: {other}"
            )));
        },
    })
}

fn open_external_url(args: Value) -> CommandOutcome {
    match parse::<OpenExternalUrlArgs>(args) {
        Ok(a) if !is_safe_external_url(&a.url) => CommandOutcome::Sync(Err(
            BridgeError::invalid_args(format!("refusing to open non-https url: {}", a.url)),
        )),
        Ok(a) => match opener::open(&a.url) {
            Ok(()) => CommandOutcome::Sync(Ok(json!({}))),
            Err(e) => CommandOutcome::Sync(Err(BridgeError::new(
                ErrorScope::Internal,
                ErrorCode::Internal,
                format!("open url failed: {e}"),
            ))),
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

pub(super) fn diagnostics_dispatch(
    app: &GuiApp,
    cmd: &str,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "diagnostics.openLogDirectory" | "openLogFolder" => {
            send(app, UiEvent::OpenLogDirectory { reply_to: reply_id });
            CommandOutcome::Async
        },
        "diagnostics.exportBundle" => {
            send(app, UiEvent::ExportDiagnosticBundle { reply_to: reply_id });
            CommandOutcome::Async
        },
        "diagnostics.info" => CommandOutcome::Sync(Ok(json!({
            "version": crate::brand::brand().version,
            "git_sha": crate::buildinfo::short_sha(),
            "git_sha_full": crate::buildinfo::GIT_SHA,
            "build_date": crate::buildinfo::GIT_COMMIT_DATE,
            "build_timestamp": crate::buildinfo::BUILD_TIMESTAMP,
            "branch": crate::buildinfo::GIT_BRANCH,
            "rendered": crate::buildinfo::render(),
        }))),
        _ => return None,
    })
}

fn marketplace_listing(app: &GuiApp) -> Value {
    let snap = app.state.snapshot();
    let listing = crate::gui::server_marketplace::build_listing(
        app.ctx.proxy.loopback(),
        &app.ctx.mcp_registry(),
        &snap.mcp_auth,
    );
    crate::gui::server_marketplace::listing_to_value(&listing).unwrap_or(Value::Null)
}
