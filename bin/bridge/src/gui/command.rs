use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::secret::Secret;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, server_json};

#[derive(Debug)]
pub enum CommandOutcome {
    Sync(Result<Value, BridgeError>),
    Async,
}

#[derive(Debug, Deserialize)]
struct GatewaySetArgs {
    url: String,
}

#[derive(Debug, Deserialize)]
struct LoginArgs {
    token: Secret,
    #[serde(default)]
    gateway: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SessionLoginArgs {
    gateway: Option<String>,
    keep_signed_in: bool,
}

#[derive(Debug, Deserialize)]
struct HostIdArgs {
    #[serde(rename = "hostId")]
    host_id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CancelArgs {
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HostInstallArgs {
    #[serde(rename = "hostId")]
    host_id: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct OpenExternalUrlArgs {
    url: String,
}

#[derive(Debug, Deserialize)]
struct HostModelFilterArgs {
    #[serde(rename = "hostId")]
    host_id: String,
    /// `None` clears the override; `Some([])` means "all models".
    #[serde(default)]
    protocols: Option<Vec<String>>,
}

fn is_safe_external_url(url: &str) -> bool {
    url.starts_with("https://")
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

fn meta_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: &Value,
    _reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "state.snapshot" => {
            CommandOutcome::Sync(Ok(server_json::snapshot_value(&app.state.snapshot())))
        },
        "marketplace.list" => CommandOutcome::Sync(Ok(marketplace_listing(app))),
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

fn gateway_dispatch(
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
            send(app, UiEvent::McpAuthProbeRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        _ => return None,
    })
}

fn auth_dispatch(
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

fn sync_dispatch(
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

fn host_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "host.probe" => host_probe(app, args, reply_id),
        "host.profile.generate" => host_profile_generate(app, args, reply_id),
        "host.profile.install" => host_profile_install(app, args, reply_id),
        "host.model-filter.set" => match parse::<HostModelFilterArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::Host(HostUiEvent::ModelFilterSetRequested {
                        host_id: a.host_id,
                        protocols: a.protocols,
                        reply_to: reply_id,
                    }),
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "host.proxy.probe" => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProxyProbeRequested { reply_to: reply_id }),
            );
            CommandOutcome::Async
        },
        _ => return None,
    })
}

fn agent_dispatch(
    app: &GuiApp,
    cmd: &str,
    args: Value,
    reply_id: ReplyId,
) -> Option<CommandOutcome> {
    Some(match cmd {
        "agent.uninstall" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentUninstall {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.openConfig" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpenConfig {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "agent.open" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::AgentOpen {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    },
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        _ => return None,
    })
}

fn diagnostics_dispatch(app: &GuiApp, cmd: &str, reply_id: ReplyId) -> Option<CommandOutcome> {
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
            "version": env!("CARGO_PKG_VERSION"),
            "git_sha": crate::cli::diagnostics::short_sha(),
            "git_sha_full": crate::cli::diagnostics::GIT_SHA,
            "build_date": crate::cli::diagnostics::GIT_COMMIT_DATE,
            "build_timestamp": crate::cli::diagnostics::BUILD_TIMESTAMP,
            "branch": crate::cli::diagnostics::GIT_BRANCH,
            "rendered": crate::cli::diagnostics::render(),
        }))),
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

fn host_probe(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostIdArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProbeRequested {
                    host_id: a.host_id,
                    cause: ProbeCause::Manual,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

fn host_profile_generate(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostIdArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProfileGenerateRequested {
                    host_id: a.host_id,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

fn host_profile_install(app: &GuiApp, args: Value, reply_id: ReplyId) -> CommandOutcome {
    match parse::<HostInstallArgs>(args) {
        Ok(a) => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProfileInstallRequested {
                    host_id: a.host_id,
                    path: a.path,
                    reply_to: reply_id,
                }),
            );
            CommandOutcome::Async
        },
        Err(e) => CommandOutcome::Sync(Err(e)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, BridgeError> {
    serde_json::from_value(args).map_err(|e| BridgeError::invalid_args(e.to_string()))
}

fn send(app: &GuiApp, event: UiEvent) {
    _ = app.proxy.send_event(event);
}

fn marketplace_listing(app: &GuiApp) -> Value {
    let snap = app.state.snapshot();
    let listing = crate::gui::server_marketplace::build_listing(&snap.mcp_auth);
    crate::gui::server_marketplace::listing_to_value(&listing).unwrap_or(Value::Null)
}

pub(crate) fn reply_for_value(result: Result<Value, BridgeError>) -> IpcReplyPayload {
    match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(e) => IpcReplyPayload::err(e),
    }
}
