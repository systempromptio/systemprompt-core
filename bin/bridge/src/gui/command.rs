use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::secret::Secret;
use crate::gui::events::{ReplyId, UiEvent};
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::state::CancelScope;
use crate::gui::{GuiApp, server_json};

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

fn is_safe_external_url(url: &str) -> bool {
    url.starts_with("https://")
}

pub(crate) fn dispatch(app: &mut GuiApp, id: u64, cmd: &str, args: Value) -> CommandOutcome {
    let reply_id: ReplyId = Some(id);
    match cmd {
        "state.snapshot" => {
            CommandOutcome::Sync(Ok(server_json::snapshot_value(&app.state.snapshot())))
        },
        "marketplace.list" => CommandOutcome::Sync(Ok(marketplace_listing())),
        "gateway.set" => match parse::<GatewaySetArgs>(args) {
            Ok(a) => {
                if a.url.trim().is_empty() {
                    return CommandOutcome::Sync(Err(BridgeError::new(
                        ErrorScope::Gateway,
                        ErrorCode::InvalidArgs,
                        "gateway url is empty",
                    )));
                }
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
        "login" => match parse::<LoginArgs>(args) {
            Ok(a) => {
                if a.token.expose().trim().is_empty() {
                    return CommandOutcome::Sync(Err(BridgeError::new(
                        ErrorScope::Identity,
                        ErrorCode::InvalidArgs,
                        "PAT is empty",
                    )));
                }
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
        "logout" => {
            send(app, UiEvent::LogoutRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "sync" => {
            send(app, UiEvent::SyncRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "validate" => {
            send(app, UiEvent::ValidateRequested { reply_to: reply_id });
            CommandOutcome::Async
        },
        "host.probe" => match parse::<HostIdArgs>(args) {
            Ok(a) => {
                send(
                    app,
                    UiEvent::Host(HostUiEvent::ProbeRequested {
                        host_id: a.host_id,
                        reply_to: reply_id,
                    }),
                );
                CommandOutcome::Async
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
        "host.profile.generate" => match parse::<HostIdArgs>(args) {
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
        },
        "host.profile.install" => match parse::<HostInstallArgs>(args) {
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
        },
        "host.proxy.probe" => {
            send(
                app,
                UiEvent::Host(HostUiEvent::ProxyProbeRequested { reply_to: reply_id }),
            );
            CommandOutcome::Async
        },
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
        "setup.complete" => {
            send(app, UiEvent::SetupComplete);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openConfigFolder" => {
            send(app, UiEvent::OpenConfigFolder);
            CommandOutcome::Sync(Ok(json!({})))
        },
        "openExternalUrl" => match parse::<OpenExternalUrlArgs>(args) {
            Ok(a) => {
                if !is_safe_external_url(&a.url) {
                    return CommandOutcome::Sync(Err(BridgeError::invalid_args(format!(
                        "refusing to open non-https url: {}",
                        a.url
                    ))));
                }
                if let Err(e) = opener::open(&a.url) {
                    return CommandOutcome::Sync(Err(BridgeError::new(
                        ErrorScope::Internal,
                        ErrorCode::Internal,
                        format!("open url failed: {e}"),
                    )));
                }
                CommandOutcome::Sync(Ok(json!({})))
            },
            Err(e) => CommandOutcome::Sync(Err(e)),
        },
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
        "cancel" => match parse::<CancelArgs>(args) {
            Ok(a) => {
                let scope = match a.scope.as_deref() {
                    None | Some("all") => None,
                    Some("sync") => Some(CancelScope::Sync),
                    Some("login") => Some(CancelScope::Login),
                    Some("gateway") | Some("gateway-probe") => Some(CancelScope::GatewayProbe),
                    Some(other) => {
                        return CommandOutcome::Sync(Err(BridgeError::invalid_args(format!(
                            "unknown cancel scope: {other}"
                        ))));
                    },
                };
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
        "quit" => {
            send(app, UiEvent::Quit);
            CommandOutcome::Sync(Ok(json!({})))
        },
        other => CommandOutcome::Sync(Err(BridgeError::new(
            ErrorScope::Internal,
            ErrorCode::NotFound,
            format!("unknown command: {other}"),
        ))),
    }
}

fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, BridgeError> {
    serde_json::from_value(args).map_err(|e| BridgeError::invalid_args(e.to_string()))
}

fn send(app: &GuiApp, event: UiEvent) {
    let _ = app.proxy.send_event(event);
}

fn marketplace_listing() -> Value {
    let listing = crate::gui::server_marketplace::build_listing();
    crate::gui::server_marketplace::listing_to_value(&listing).unwrap_or(Value::Null)
}

pub(crate) fn reply_for_value(result: Result<Value, BridgeError>) -> IpcReplyPayload {
    match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(e) => IpcReplyPayload::err(e),
    }
}
