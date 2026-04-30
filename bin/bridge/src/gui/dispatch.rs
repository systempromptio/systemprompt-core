use winit::event_loop::ActiveEventLoop;

use crate::gui::events::UiEvent;
use crate::gui::{GuiApp, handlers};

fn event_kind(event: &UiEvent) -> &'static str {
    match event {
        UiEvent::OpenSettings => "OpenSettings",
        UiEvent::SyncRequested { .. } => "SyncRequested",
        UiEvent::ValidateRequested { .. } => "ValidateRequested",
        UiEvent::OpenConfigFolder => "OpenConfigFolder",
        UiEvent::OpenLogDirectory { .. } => "OpenLogDirectory",
        UiEvent::ExportDiagnosticBundle { .. } => "ExportDiagnosticBundle",
        UiEvent::LoginRequested { .. } => "LoginRequested",
        UiEvent::LogoutRequested { .. } => "LogoutRequested",
        UiEvent::SetGatewayRequested { .. } => "SetGatewayRequested",
        UiEvent::GatewayProbeRequested { .. } => "GatewayProbeRequested",
        UiEvent::Quit => "Quit",
        UiEvent::SyncStarted => "SyncStarted",
        UiEvent::SyncFinished { .. } => "SyncFinished",
        UiEvent::ValidateFinished { .. } => "ValidateFinished",
        UiEvent::LoginFinished { .. } => "LoginFinished",
        UiEvent::LogoutFinished { .. } => "LogoutFinished",
        UiEvent::SetGatewayFinished { .. } => "SetGatewayFinished",
        UiEvent::GatewayProbeFinished { .. } => "GatewayProbeFinished",
        UiEvent::StateRefreshed => "StateRefreshed",
        UiEvent::AgentUninstall { .. } => "AgentUninstall",
        UiEvent::AgentOpenConfig { .. } => "AgentOpenConfig",
        UiEvent::SetupComplete => "SetupComplete",
        UiEvent::FocusWindow => "FocusWindow",
        UiEvent::Host(_) => "Host",
        UiEvent::IpcInbound(_) => "IpcInbound",
        UiEvent::IpcEmit { .. } => "IpcEmit",
        UiEvent::IpcReply { .. } => "IpcReply",
        UiEvent::ProxyStatsTick => "ProxyStatsTick",
    }
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
    match event {
        UiEvent::OpenSettings => handlers::settings::on_open_settings(app, event_loop),
        UiEvent::SyncRequested { reply_to } => handlers::sync::on_sync_requested(app, reply_to),
        UiEvent::ValidateRequested { reply_to } => {
            handlers::validate::on_validate_requested(app, reply_to)
        },
        UiEvent::OpenConfigFolder => handlers::settings::on_open_config_folder(app),
        UiEvent::OpenLogDirectory { reply_to } => {
            handlers::diagnostics::on_open_log_directory(app, reply_to)
        },
        UiEvent::ExportDiagnosticBundle { reply_to } => {
            handlers::diagnostics::on_export_diagnostic_bundle(app, reply_to)
        },
        UiEvent::FocusWindow => {
            if let Some(win) = &app.settings_window {
                win.focus();
            } else {
                handlers::settings::on_open_settings(app, event_loop);
            }
        },
        UiEvent::LoginRequested {
            token,
            gateway,
            reply_to,
        } => handlers::auth::on_login_requested(app, &token, gateway, reply_to),
        UiEvent::LogoutRequested { reply_to } => {
            handlers::auth::on_logout_requested(app, reply_to)
        },
        UiEvent::SetGatewayRequested { url, reply_to } => {
            handlers::auth::on_set_gateway_requested(app, &url, reply_to)
        },
        UiEvent::Quit => handlers::quit::on_quit(),
        UiEvent::SyncStarted => handlers::sync::on_sync_started(app),
        UiEvent::SyncFinished { result, reply_to } => {
            handlers::sync::on_sync_finished(app, result, reply_to)
        },
        UiEvent::ValidateFinished { report, reply_to } => {
            handlers::validate::on_validate_finished(app, report, reply_to)
        },
        UiEvent::LoginFinished { result, reply_to } => {
            handlers::auth::on_login_finished(app, result, reply_to)
        },
        UiEvent::LogoutFinished { result, reply_to } => {
            handlers::auth::on_logout_finished(app, result, reply_to)
        },
        UiEvent::SetGatewayFinished { result, reply_to } => {
            handlers::auth::on_set_gateway_finished(app, result, reply_to)
        },
        UiEvent::GatewayProbeRequested { reply_to } => {
            handlers::gateway_probe::on_gateway_probe_requested(app, reply_to)
        },
        UiEvent::GatewayProbeFinished { outcome, reply_to } => {
            handlers::gateway_probe::on_gateway_probe_finished(app, outcome, reply_to)
        },
        UiEvent::StateRefreshed => handlers::state::on_state_refreshed(app),
        UiEvent::AgentUninstall { host_id, reply_to } => {
            handlers::agents::on_uninstall(app, &host_id, reply_to)
        },
        UiEvent::AgentOpenConfig { host_id, reply_to } => {
            handlers::agents::on_open_config(app, &host_id, reply_to)
        },
        UiEvent::SetupComplete => handlers::agents::on_setup_complete(app),

        UiEvent::Host(e) => crate::gui::hosts::dispatch::handle(app, e),

        UiEvent::IpcInbound(raw) => crate::gui::ipc_runtime::handle_inbound(app, raw),
        UiEvent::IpcEmit { channel, payload } => {
            crate::gui::ipc_runtime::send_emit(app, channel, &payload)
        },
        UiEvent::IpcReply { id, payload, ok } => {
            crate::gui::ipc_runtime::send_reply(app, id, payload, ok)
        },
        UiEvent::ProxyStatsTick => crate::gui::ipc_runtime::emit_proxy_stats(app),
    }
}
