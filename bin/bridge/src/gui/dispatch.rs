//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use winit::event_loop::ActiveEventLoop;

use crate::gui::events::UiEvent;
use crate::gui::{GuiApp, handlers};

fn event_kind(event: &UiEvent) -> &'static str {
    request_kind(event)
        .or_else(|| finish_kind(event))
        .or_else(|| lifecycle_kind(event))
        .or_else(|| ipc_kind(event))
        .unwrap_or("Unknown")
}

const fn request_kind(event: &UiEvent) -> Option<&'static str> {
    Some(match event {
        UiEvent::OpenSettings => "OpenSettings",
        UiEvent::SyncRequested { .. } => "SyncRequested",
        UiEvent::ValidateRequested { .. } => "ValidateRequested",
        UiEvent::OpenConfigFolder => "OpenConfigFolder",
        UiEvent::OpenLogDirectory { .. } => "OpenLogDirectory",
        UiEvent::ExportDiagnosticBundle { .. } => "ExportDiagnosticBundle",
        UiEvent::LoginRequested { .. } => "LoginRequested",
        UiEvent::SessionLoginRequested { .. } => "SessionLoginRequested",
        UiEvent::LogoutRequested { .. } => "LogoutRequested",
        UiEvent::SetGatewayRequested { .. } => "SetGatewayRequested",
        UiEvent::GatewayProbeRequested { .. } => "GatewayProbeRequested",
        UiEvent::McpAuthProbeRequested { .. } => "McpAuthProbeRequested",
        UiEvent::ProfileFetchRequested { .. } => "ProfileFetchRequested",
        _ => return None,
    })
}

const fn finish_kind(event: &UiEvent) -> Option<&'static str> {
    Some(match event {
        UiEvent::SyncStarted => "SyncStarted",
        UiEvent::SyncFinished { .. } => "SyncFinished",
        UiEvent::ValidateFinished { .. } => "ValidateFinished",
        UiEvent::LoginFinished { .. } => "LoginFinished",
        UiEvent::SessionLoginFinished { .. } => "SessionLoginFinished",
        UiEvent::LogoutFinished { .. } => "LogoutFinished",
        UiEvent::SetGatewayFinished { .. } => "SetGatewayFinished",
        UiEvent::GatewayProbeFinished { .. } => "GatewayProbeFinished",
        UiEvent::McpAuthProbeFinished { .. } => "McpAuthProbeFinished",
        UiEvent::ProfileFetchFinished { .. } => "ProfileFetchFinished",
        _ => return None,
    })
}

const fn lifecycle_kind(event: &UiEvent) -> Option<&'static str> {
    Some(match event {
        UiEvent::Quit => "Quit",
        UiEvent::StateRefreshed => "StateRefreshed",
        UiEvent::AgentUninstall { .. } => "AgentUninstall",
        UiEvent::AgentOpenConfig { .. } => "AgentOpenConfig",
        UiEvent::AgentOpen { .. } => "AgentOpen",
        UiEvent::SetupComplete => "SetupComplete",
        UiEvent::FocusWindow => "FocusWindow",
        UiEvent::Host(_) => "Host",
        UiEvent::ProxyStatsTick => "ProxyStatsTick",
        _ => return None,
    })
}

const fn ipc_kind(event: &UiEvent) -> Option<&'static str> {
    Some(match event {
        UiEvent::IpcInbound(_) => "IpcInbound",
        UiEvent::IpcEmit { .. } => "IpcEmit",
        UiEvent::IpcReply { .. } => "IpcReply",
        UiEvent::CancelInFlight { .. } => "CancelInFlight",
        _ => return None,
    })
}

#[tracing::instrument(
    level = "info",
    name = "gui_dispatch",
    skip(app, event_loop, event),
    fields(
        event_kind = event_kind(&event),
        request_id = %uuid::Uuid::new_v4(),
    ),
)]
pub(crate) fn dispatch(app: &mut GuiApp, event_loop: &ActiveEventLoop, event: UiEvent) {
    tracing::trace!(?event, "ui dispatch");
    let event = match dispatch_window(app, event_loop, event) {
        Ok(()) => return,
        Err(e) => *e,
    };
    let event = match dispatch_request(app, event) {
        Ok(()) => return,
        Err(e) => *e,
    };
    let event = match dispatch_finished(app, event) {
        Ok(()) => return,
        Err(e) => *e,
    };
    let event = match dispatch_lifecycle(app, event) {
        Ok(()) => return,
        Err(e) => *e,
    };
    dispatch_ipc(app, event);
}

fn dispatch_window(
    app: &mut GuiApp,
    event_loop: &ActiveEventLoop,
    event: UiEvent,
) -> Result<(), Box<UiEvent>> {
    match event {
        UiEvent::OpenSettings => handlers::settings::on_open_settings(app, event_loop),
        UiEvent::OpenConfigFolder => handlers::settings::on_open_config_folder(app),
        UiEvent::FocusWindow => {
            if let Some(win) = &app.settings_window {
                win.focus();
            } else {
                handlers::settings::on_open_settings(app, event_loop);
            }
        },
        other => return Err(Box::new(other)),
    }
    Ok(())
}

fn dispatch_request(app: &mut GuiApp, event: UiEvent) -> Result<(), Box<UiEvent>> {
    match event {
        UiEvent::SyncRequested { reply_to } => handlers::sync::on_sync_requested(app, reply_to),
        UiEvent::ValidateRequested { reply_to } => {
            handlers::validate::on_validate_requested(app, reply_to);
        },
        UiEvent::OpenLogDirectory { reply_to } => {
            handlers::diagnostics::on_open_log_directory(app, reply_to);
        },
        UiEvent::ExportDiagnosticBundle { reply_to } => {
            handlers::diagnostics::on_export_diagnostic_bundle(app, reply_to);
        },
        UiEvent::LoginRequested {
            token,
            gateway,
            reply_to,
        } => handlers::auth::on_login_requested(app, &token, gateway, reply_to),
        UiEvent::SessionLoginRequested {
            gateway,
            keep_signed_in,
            reply_to,
        } => {
            handlers::auth::on_session_login_requested(app, gateway, keep_signed_in, reply_to);
        },
        UiEvent::LogoutRequested { reply_to } => handlers::auth::on_logout_requested(app, reply_to),
        UiEvent::SetGatewayRequested { url, reply_to } => {
            handlers::auth::on_set_gateway_requested(app, &url, reply_to);
        },
        UiEvent::GatewayProbeRequested { reply_to } => {
            handlers::gateway_probe::on_gateway_probe_requested(app, reply_to);
        },
        UiEvent::McpAuthProbeRequested { reply_to } => {
            handlers::mcp_auth_probe::on_mcp_auth_probe_requested(app, reply_to);
        },
        UiEvent::ProfileFetchRequested { reply_to } => {
            handlers::profile::on_profile_fetch_requested(app, reply_to);
        },
        other => return Err(Box::new(other)),
    }
    Ok(())
}

fn dispatch_finished(app: &mut GuiApp, event: UiEvent) -> Result<(), Box<UiEvent>> {
    match event {
        UiEvent::SyncFinished { result, reply_to } => {
            handlers::sync::on_sync_finished(app, result, reply_to);
        },
        UiEvent::ValidateFinished { report, reply_to } => {
            handlers::validate::on_validate_finished(app, report, reply_to);
        },
        UiEvent::LoginFinished { result, reply_to } => {
            handlers::auth::on_login_finished(app, result, reply_to);
        },
        UiEvent::SessionLoginFinished { result, reply_to } => {
            // Same post-auth path as a PAT login: reload, probe identity, sync.
            handlers::auth::on_login_finished(app, result, reply_to);
        },
        UiEvent::LogoutFinished { result, reply_to } => {
            handlers::auth::on_logout_finished(app, result, reply_to);
        },
        UiEvent::SetGatewayFinished { result, reply_to } => {
            handlers::auth::on_set_gateway_finished(app, result, reply_to);
        },
        UiEvent::GatewayProbeFinished { outcome, reply_to } => {
            handlers::gateway_probe::on_gateway_probe_finished(app, outcome, reply_to);
        },
        UiEvent::McpAuthProbeFinished { results, reply_to } => {
            handlers::mcp_auth_probe::on_mcp_auth_probe_finished(app, results, reply_to);
        },
        UiEvent::ProfileFetchFinished { result, reply_to } => {
            handlers::profile::on_profile_fetch_finished(app, result, reply_to);
        },
        other => return Err(Box::new(other)),
    }
    Ok(())
}

fn dispatch_lifecycle(app: &mut GuiApp, event: UiEvent) -> Result<(), Box<UiEvent>> {
    match event {
        UiEvent::Quit => handlers::quit::on_quit(),
        UiEvent::SyncStarted => handlers::sync::on_sync_started(app),
        UiEvent::StateRefreshed => handlers::state::on_state_refreshed(app),
        UiEvent::AgentUninstall { host_id, reply_to } => {
            handlers::agents::on_uninstall(app, &host_id, reply_to);
        },
        UiEvent::AgentOpenConfig { host_id, reply_to } => {
            handlers::agents::on_open_config(app, &host_id, reply_to);
        },
        UiEvent::AgentOpen { host_id, reply_to } => {
            handlers::agents::on_open(app, &host_id, reply_to);
        },
        UiEvent::SetupComplete => handlers::agents::on_setup_complete(app),
        UiEvent::Host(e) => crate::gui::hosts::dispatch::handle(app, e),
        UiEvent::ProxyStatsTick => crate::gui::emit::emit_proxy_stats(app),
        other => return Err(Box::new(other)),
    }
    Ok(())
}

fn dispatch_ipc(app: &GuiApp, event: UiEvent) {
    match event {
        UiEvent::IpcInbound(raw) => crate::gui::ipc_runtime::handle_inbound(app, &raw),
        UiEvent::IpcEmit { channel, payload } => {
            crate::gui::emit::send_emit(app, channel, &payload);
        },
        UiEvent::IpcReply { id, payload, ok } => crate::gui::emit::send_reply(app, id, payload, ok),
        UiEvent::CancelInFlight { scope, reply_to } => {
            handlers::cancel::on_cancel_in_flight(app, scope, reply_to);
        },
        _ => unreachable!("event should have been handled by an earlier dispatcher"),
    }
}
