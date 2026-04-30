use std::sync::Arc;

use crate::auth::secret::Secret;
use crate::gui::error::GuiError;
use crate::gui::state::GatewayProbeOutcome;
use crate::sync::SyncSummary;
use crate::validate::ValidationReport;


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
    SyncFinished(Result<SyncSummary, Arc<GuiError>>),
    ValidateFinished(ValidationReport),
    LoginFinished(Result<(), Arc<GuiError>>),
    LogoutFinished(Result<(), Arc<GuiError>>),
    SetGatewayFinished(Result<(), Arc<GuiError>>),
    GatewayProbeFinished(GatewayProbeOutcome),
    StateRefreshed,

    AgentUninstall { host_id: String },
    AgentOpenConfig { host_id: String },
    SetupComplete,

    Host(HostUiEvent),
}
