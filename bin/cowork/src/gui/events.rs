use crate::gui::state::GatewayProbeOutcome;
use crate::secret::Secret;
use crate::sync::SyncSummary;
use crate::validate::ValidationReport;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::gui::claude::events::ClaudeUiEvent;

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
    SyncFinished(Result<SyncSummary, String>),
    ValidateFinished(ValidationReport),
    LoginFinished(Result<(), String>),
    LogoutFinished(Result<(), String>),
    SetGatewayFinished(Result<(), String>),
    GatewayProbeFinished(GatewayProbeOutcome),
    StateRefreshed,

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Claude(ClaudeUiEvent),
}
