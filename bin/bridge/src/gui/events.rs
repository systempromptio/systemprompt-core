//! `UiEvent` definitions carried between GUI components.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::Value;

use crate::auth::secret::Secret;
use crate::gui::error::GuiError;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::state::{CancelScope, GatewayProbeOutcome};
use crate::ids::HostId;
use crate::proxy::mcp_probe::McpServerAuth;

/// What one probe pass produced: every registered server, or one re-checked
/// server (`None` when the registry did not know the id).
#[derive(Debug, Clone)]
pub enum McpProbeResults {
    All(Vec<McpServerAuth>),
    One(Option<McpServerAuth>),
}
use crate::sync::SyncSummary;
use crate::validate::ValidationReport;

pub type ReplyId = Option<u64>;

#[derive(Debug, Clone)]
pub enum UiEvent {
    OpenSettings,
    SyncRequested {
        reply_to: ReplyId,
    },
    ValidateRequested {
        reply_to: ReplyId,
    },
    OpenConfigFolder,
    OpenLogDirectory {
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
    SyncStep(crate::sync::progress::SyncProgress),
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
    SetGatewayFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    GatewayProbeFinished {
        // Why: `None` when the probe was cancelled or superseded: it concluded
        // nothing, so there is no outcome to apply.
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
        result: Result<Value, Arc<GuiError>>,
        reply_to: ReplyId,
    },

    UpdateCheckRequested {
        reply_to: ReplyId,
    },
    UpdateCheckFinished {
        result: Result<Value, Arc<GuiError>>,
        reply_to: ReplyId,
    },
    UpdateInstallRequested {
        reply_to: ReplyId,
    },
    UpdateInstallFinished {
        result: Result<Value, Arc<GuiError>>,
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
    SettingsWriteRequested {
        key: String,
        value: Value,
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
        payload: Value,
    },
    IpcReply {
        id: u64,
        payload: Value,
        ok: bool,
    },
    ProxyStatsTick,
    CancelInFlight {
        scope: Option<CancelScope>,
        reply_to: ReplyId,
    },
}
