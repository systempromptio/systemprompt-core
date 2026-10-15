//! The agent-health verdict vocabulary: state, reason and fix action.
//!
//! Serialized codes are the localisation key suffixes the GUI looks up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::integration::host_app::StaleReason;
use crate::proxy_probe::ProxyProbeState;
use crate::verdict::Tone;

/// Host configuration, application and proxy health state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentState {
    Working,
    Ready,
    Attention,
    NeedsApproval,
    ApprovalExpired,
    NeedsAdminApproval,
    NeedsReapply,
    RestartNeeded,
    NotSetUp,
    NotInstalled,
    Down,
    Checking,
}

impl AgentState {
    // Why: an app that is not on this computer is a fact about the computer,
    // not a fault in the agent, so it reads neutral and never asks for
    // attention.
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Working | Self::Ready => Tone::Ok,
            Self::Attention
            | Self::NeedsApproval
            | Self::ApprovalExpired
            | Self::NeedsAdminApproval
            | Self::NeedsReapply
            | Self::RestartNeeded
            | Self::NotSetUp => Tone::Warn,
            Self::Down => Tone::Err,
            Self::Checking | Self::NotInstalled => Tone::Unknown,
        }
    }

    // Why: Something on this machine waits on the user: every state with a fix
    // button except a missing app, which is a fact rather than a fault.
    #[must_use]
    pub const fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::Attention
                | Self::NeedsApproval
                | Self::ApprovalExpired
                | Self::NeedsAdminApproval
                | Self::NeedsReapply
                | Self::RestartNeeded
        )
    }
}

/// Why the agent is in that state, with the arguments its message needs.
///
/// `CloudManaged` is routed through the gateway centrally; `NotRouted` is a
/// sync-only CLI whose settings do not send its inference to the gateway.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentReason {
    Governed { when_unix: Option<u64> },
    Awaiting,
    AppMissing,
    LeftoverConfig,
    AwaitingOsApproval,
    ApprovalExpired,
    RestartPending,
    Stale { cause: StaleReason, keys: String },
    SyncFailed { detail: String },
    Partial { missing: String },
    Unverifiable { detail: String },
    Absent,
    NoKey { providers: String },
    NoModels,
    ProxyDown { probe: ProxyProbeState },
    NeverProbed,
    CloudManaged,
    NotRouted,
}

/// The one button that fixes this state, if there is one.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentAction {
    Download,
    Repair,
    Update,
    UpdateAdmin,
    Verify,
    Open,
    Add,
    Enrol,
    Connect,
    OpenDeviceManagement,
    Restart,
    RemoveLeftover,
    OfferAgain,
    ShowProfileFile,
}

/// The verdict for one agent, plus the three facts the GUI renders around it.
///
/// `is_set_up` means the agent belongs in the Agents list rather than behind
/// "Add agent" (merely "not absent", so partial and stale profiles count);
/// `is_installed` means the managed profile is present and complete;
/// `is_running` is the raw process-table fact — the app is open — and says
/// nothing about whether it is configured, so it is never the headline.
/// `is_listed` puts the row on the Agents list: every app installed here, and
/// any agent with settings of ours left behind; only an absent app with nothing
/// left over is hidden. `action` is the one fix button; `secondary_action` is
/// the second choice a missing app with leftover settings offers.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct AgentVerdict {
    pub state: AgentState,
    pub tone: Tone,
    pub reason: AgentReason,
    pub action: Option<AgentAction>,
    pub secondary_action: Option<AgentAction>,
    pub is_set_up: bool,
    pub is_listed: bool,
    pub is_installed: bool,
    pub is_running: bool,
}
