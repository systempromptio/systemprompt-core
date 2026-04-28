use crate::gui::state::GatewayProbeOutcome;
use crate::secret::Secret;
use crate::sync::SyncSummary;
use crate::validate::ValidationReport;

#[cfg(target_os = "macos")]
use crate::integration::claude_desktop::{ClaudeIntegrationSnapshot, GeneratedProfile};

#[derive(Debug, Clone)]
pub enum UiEvent {
    OpenSettings,
    SyncRequested,
    ValidateRequested,
    OpenConfigFolder,
    LoginRequested { token: Secret, gateway: Option<String> },
    LogoutRequested,
    GatewayProbeRequested,
    Quit,

    SyncStarted,
    SyncFinished(Result<SyncSummary, String>),
    ValidateFinished(ValidationReport),
    LoginFinished(Result<(), String>),
    LogoutFinished(Result<(), String>),
    GatewayProbeFinished(GatewayProbeOutcome),
    StateRefreshed,

    #[cfg(target_os = "macos")]
    ClaudeProbeRequested,
    #[cfg(target_os = "macos")]
    ClaudeProbeFinished(ClaudeIntegrationSnapshot),
    #[cfg(target_os = "macos")]
    ClaudeProfileGenerateRequested,
    #[cfg(target_os = "macos")]
    ClaudeProfileGenerateFinished(Result<GeneratedProfile, String>),
    #[cfg(target_os = "macos")]
    ClaudeProfileInstallRequested(String),
    #[cfg(target_os = "macos")]
    ClaudeProfileInstallFinished(Result<String, String>),
}
