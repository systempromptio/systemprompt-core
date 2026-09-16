//! `UiEvent` definitions carried between GUI components.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::auth::secret::Secret;
use crate::gui::error::GuiError;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::state::{CancelScope, GatewayProbeOutcome};
use crate::ids::HostId;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::sync::SyncSummary;
use crate::update::UpdateUiState;
use crate::validate::ValidationReport;
use crate::wire::DeviceAction;
use crate::wire::ipc::ReplyTarget;
use crate::wire::profile::ProfileView;

/// What one probe pass produced: every registered server, or one re-checked
/// server (`None` when the registry did not know the id).
#[derive(Debug, Clone)]
pub enum McpProbeResults {
    All(Vec<McpServerAuth>),
    One(Option<McpServerAuth>),
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledUpdate {
    pub version: String,
    pub path: PathBuf,
}

pub type ReplyId = Option<ReplyTarget>;

#[derive(Debug, Clone)]
pub enum UiEvent {
    OpenSettings,
    OpenDeviceAction(DeviceAction),
    SyncRequested {
        reply_to: ReplyId,
    },
    ValidateRequested {
        reply_to: ReplyId,
    },
    OpenConfigFolder,
    RevealApplication,
    OpenLogDirectory {
        reply_to: ReplyId,
    },
    ProxySecretResetRequested {
        reply_to: ReplyId,
    },
    ConfigDirRepairRequested {
        reply_to: ReplyId,
    },
    ConfigDirRepairFinished {
        result: Result<String, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    ExportDiagnosticBundle {
        reply_to: ReplyId,
    },
    FocusWindow,
    LoginRequested {
        token: Secret,
        gateway: Option<String>,
        reply_to: ReplyId,
    },
    SessionLoginRequested {
        gateway: Option<String>,
        keep_signed_in: bool,
        reply_to: ReplyId,
    },
    LogoutRequested {
        reply_to: ReplyId,
    },
    PurgeRequested {
        reply_to: ReplyId,
    },
    DisconnectRequested {
        reply_to: ReplyId,
    },
    CredentialRejected {
        reason: String,
    },
    SetGatewayRequested {
        url: String,
        reply_to: ReplyId,
    },
    GatewayProbeRequested {
        reply_to: ReplyId,
    },
    McpAuthProbeRequested {
        server_id: Option<String>,
        reply_to: ReplyId,
    },
    Quit,

    SyncStarted,
    SyncStep(crate::progress::SyncProgress),
    SyncFinished {
        result: Result<SyncSummary, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    ValidateFinished {
        report: ValidationReport,
        reply_to: ReplyId,
    },
    LoginFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    SessionLoginFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    LogoutFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    PurgeFinished {
        result: Result<Vec<String>, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    DisconnectFinished {
        result: Result<Vec<String>, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    SetGatewayFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    GatewayProbeFinished {
        outcome: Option<GatewayProbeOutcome>,
        reply_to: ReplyId,
    },
    McpAuthProbeFinished {
        results: McpProbeResults,
        reply_to: ReplyId,
    },
    StateRefreshed,

    ProfileFetchRequested {
        reply_to: ReplyId,
    },
    ProfileFetchFinished {
        result: Box<Result<ProfileView, Arc<GuiError>>>,
        reply_to: ReplyId,
    },

    UpdateCheckRequested {
        reply_to: ReplyId,
    },
    UpdateCheckFinished {
        result: Result<UpdateUiState, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    UpdateInstallRequested {
        reply_to: ReplyId,
    },
    UpdateInstallFinished {
        result: Result<InstalledUpdate, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    UpdateProgress {
        version: String,
        percent: u8,
    },
    UpdateRestartRequested,

    AutostartToggleRequested,
    SettingsReadRequested {
        reply_to: ReplyId,
    },

    AgentUninstall {
        host_id: HostId,
        reply_to: ReplyId,
    },
    AgentOpenConfig {
        host_id: HostId,
        reply_to: ReplyId,
    },
    AgentOpen {
        host_id: HostId,
        reply_to: ReplyId,
    },
    SetupComplete,
    FirstRunStart,

    Host(HostUiEvent),

    IpcInbound(String),
    IpcEmit {
        channel: &'static str,
        // JSON: webview IPC envelope, the channel's payload serialized by the emitter
        payload: Value,
    },
    ProxyStatsTick,
    CancelInFlight {
        scope: Option<CancelScope>,
        reply_to: ReplyId,
    },
}
