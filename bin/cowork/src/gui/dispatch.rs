use winit::event_loop::ActiveEventLoop;

use crate::gui::events::UiEvent;
use crate::gui::{GuiApp, handlers};

#[tracing::instrument(skip(app, event_loop), fields(event = std::any::type_name::<UiEvent>()))]
pub(crate) fn dispatch(app: &mut GuiApp, event_loop: &ActiveEventLoop, event: UiEvent) {
    tracing::trace!(?event, "ui dispatch");
    match event {
        UiEvent::OpenSettings => handlers::settings::on_open_settings(app, event_loop),
        UiEvent::SyncRequested => handlers::sync::on_sync_requested(app),
        UiEvent::ValidateRequested => handlers::validate::on_validate_requested(app),
        UiEvent::OpenConfigFolder => handlers::settings::on_open_config_folder(app),
        UiEvent::LoginRequested { token, gateway } => {
            handlers::auth::on_login_requested(app, &token, gateway)
        },
        UiEvent::LogoutRequested => handlers::auth::on_logout_requested(app),
        UiEvent::SetGatewayRequested(url) => handlers::auth::on_set_gateway_requested(app, &url),
        UiEvent::Quit => handlers::quit::on_quit(),
        UiEvent::SyncStarted => handlers::sync::on_sync_started(app),
        UiEvent::SyncFinished(r) => handlers::sync::on_sync_finished(app, r),
        UiEvent::ValidateFinished(r) => handlers::validate::on_validate_finished(app, r),
        UiEvent::LoginFinished(r) => handlers::auth::on_login_finished(app, r),
        UiEvent::LogoutFinished(r) => handlers::auth::on_logout_finished(app, r),
        UiEvent::SetGatewayFinished(r) => handlers::auth::on_set_gateway_finished(app, r),
        UiEvent::GatewayProbeRequested => handlers::gateway_probe::on_gateway_probe_requested(app),
        UiEvent::GatewayProbeFinished(o) => {
            handlers::gateway_probe::on_gateway_probe_finished(app, o)
        },
        UiEvent::StateRefreshed => handlers::state::on_state_refreshed(app),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        UiEvent::Host(e) => crate::gui::hosts::dispatch::handle(app, e),
    }
}
