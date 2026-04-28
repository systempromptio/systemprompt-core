use crate::gui::events::UiEvent;
use crate::gui::{GuiApp, handlers};

pub(crate) fn dispatch(app: &mut GuiApp, event: UiEvent) {
    match event {
        UiEvent::OpenSettings => handlers::settings::on_open_settings(app),
        UiEvent::SyncRequested => handlers::sync::on_sync_requested(app),
        UiEvent::ValidateRequested => handlers::validate::on_validate_requested(app),
        UiEvent::OpenConfigFolder => handlers::settings::on_open_config_folder(app),
        UiEvent::LoginRequested { token, gateway } => {
            handlers::auth::on_login_requested(app, token, gateway)
        },
        UiEvent::LogoutRequested => handlers::auth::on_logout_requested(app),
        UiEvent::Quit => handlers::quit::on_quit(),
        UiEvent::SyncStarted => handlers::sync::on_sync_started(app),
        UiEvent::SyncFinished(r) => handlers::sync::on_sync_finished(app, r),
        UiEvent::ValidateFinished(r) => handlers::validate::on_validate_finished(app, r),
        UiEvent::LoginFinished(r) => handlers::auth::on_login_finished(app, r),
        UiEvent::LogoutFinished(r) => handlers::auth::on_logout_finished(app, r),
        UiEvent::GatewayProbeRequested => handlers::gateway_probe::on_gateway_probe_requested(app),
        UiEvent::GatewayProbeFinished(o) => {
            handlers::gateway_probe::on_gateway_probe_finished(app, o)
        },
        UiEvent::StateRefreshed => handlers::state::on_state_refreshed(app),
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProbeRequested => handlers::claude::on_claude_probe_requested(app),
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProbeFinished(s) => handlers::claude::on_claude_probe_finished(app, s),
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProfileGenerateRequested => {
            handlers::claude::on_claude_profile_generate_requested(app)
        },
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProfileGenerateFinished(r) => {
            handlers::claude::on_claude_profile_generate_finished(app, r)
        },
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProfileInstallRequested(p) => {
            handlers::claude::on_claude_profile_install_requested(app, p)
        },
        #[cfg(target_os = "macos")]
        UiEvent::ClaudeProfileInstallFinished(r) => {
            handlers::claude::on_claude_profile_install_finished(app, r)
        },
    }
}
