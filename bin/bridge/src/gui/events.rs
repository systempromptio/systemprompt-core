use std::sync::Arc;

use serde_json::Value;

use crate::auth::secret::Secret;
use crate::gui::error::GuiError;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::state::GatewayProbeOutcome;
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
    LogoutRequested {
        reply_to: ReplyId,
    },
    SetGatewayRequested {
        url: String,
        reply_to: ReplyId,
    },
    GatewayProbeRequested {
        reply_to: ReplyId,
    },
    Quit,

    SyncStarted,
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
    LogoutFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    SetGatewayFinished {
        result: Result<(), Arc<GuiError>>,
        reply_to: ReplyId,
    },
    GatewayProbeFinished {
        outcome: GatewayProbeOutcome,
        reply_to: ReplyId,
    },
    StateRefreshed,

    AgentUninstall {
        host_id: String,
        reply_to: ReplyId,
    },
    AgentOpenConfig {
        host_id: String,
        reply_to: ReplyId,
    },
    SetupComplete,

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
}
