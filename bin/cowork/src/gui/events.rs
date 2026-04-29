use crate::auth::secret::Secret;
use crate::gui::error::GuiError;
use crate::gui::state::GatewayProbeOutcome;
use crate::sync::SyncSummary;
use crate::validate::ValidationReport;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::gui::hosts::events::HostUiEvent;

#[derive(Debug, Clone)]
pub enum UiEvent {
    OpenSettings,
    SyncRequested,
    ValidateRequested,
    OpenConfigFolder,
    LoginRequested {
        token: Secret,
        gateway: Option<String>,
    },
    LogoutRequested,
    SetGatewayRequested(String),
    GatewayProbeRequested,
    Quit,

    SyncStarted,
    SyncFinished(Result<SyncSummary, GuiError>),
    ValidateFinished(ValidationReport),
    LoginFinished(Result<(), GuiError>),
    LogoutFinished(Result<(), GuiError>),
    SetGatewayFinished(Result<(), GuiError>),
    GatewayProbeFinished(GatewayProbeOutcome),
    StateRefreshed,

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Host(HostUiEvent),
}
