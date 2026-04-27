use crate::sync::SyncSummary;
use crate::validate::ValidationReport;

#[derive(Debug, Clone)]
pub enum UiEvent {
    OpenSettings,
    SyncRequested,
    ValidateRequested,
    OpenConfigFolder,
    LoginRequested { token: String, gateway: Option<String> },
    LogoutRequested,
    Quit,

    SyncStarted,
    SyncFinished(Result<SyncSummary, String>),
    ValidateFinished(ValidationReport),
    LoginFinished(Result<(), String>),
    LogoutFinished(Result<(), String>),
    StateRefreshed,
}
