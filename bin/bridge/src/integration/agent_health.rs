//! The single derivation of what a host's state *means* to the reader.
//!
//! This used to live in JavaScript, three times over: the Agents list, the
//! Status summary card and the overall badge each folded the same snapshot
//! their own way, with their own `|| "absent"` fallback and their own view of
//! whether the proxy mattered. They disagreed — the Agents tab could report
//! "Not working" while the summary card said "all configured".
//!
//! So the verdict is computed here, once, over the typed inputs Rust already
//! owns, and the GUI renders it. Every enum serialises to a kebab code that is
//! also the localisation key suffix, which keeps user-facing copy in the FTL
//! catalogue while leaving no branching in the renderer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::integration::host_app::{AppInstallState, ProfileState, StaleReason};
use crate::proxy_probe::{ProxyHealth, ProxyProbeState};
use crate::verdict::Tone;

/// What the reader is told about one agent.
///
/// `Working` is governed and proven (profile installed, app present, proxy
/// answering); `Ready` is installed and
/// correct but never launched; `Attention` is a specific, fixable local fault;
/// `NotSetUp` means no profile here — a thing you may add, not a thing that is
/// broken; `Down` should work but the local proxy is not answering; `Checking`
/// is never probed, or a probe in flight — NOT evidence of absence.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentState {
    Working,
    Ready,
    Attention,
    NotSetUp,
    Down,
    Checking,
}

impl AgentState {
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Working | Self::Ready => Tone::Ok,
            Self::Attention | Self::NotSetUp => Tone::Warn,
            Self::Down => Tone::Err,
            Self::Checking => Tone::Unknown,
        }
    }
}

/// Why the agent is in that state. Carries the arguments its message needs;
/// `CloudManaged` is routed through the gateway centrally with nothing to
/// install on this machine.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentReason {
    Governed { when_unix: Option<u64> },
    Awaiting,
    AppMissing,
    Stale { cause: StaleReason },
    Partial { missing: String },
    Absent,
    NoKey { providers: String },
    NoModels,
    ProxyDown { probe: ProxyProbeState },
    NeverProbed,
    CloudManaged,
}

/// The one button that fixes this state, if there is one.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentAction {
    Download,
    Repair,
    Verify,
    Open,
    Add,
}

/// Whether this agent is configured on this machine at all.
///
/// `LocalProfile` has a managed profile this bridge installs and probes
/// locally; `SyncOnly` is
/// gateway-enabled with no local profile — it syncs, it does not install.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum AgentSurface {
    LocalProfile,
    SyncOnly,
}

/// What the GUI may offer for one agent, derived from its surface.
///
/// The front end renders these; it does not decide them. Before this existed
/// the agent drawer offered Open / Repair / Verify / Show config file / Remove
/// for every row unconditionally, so a [`AgentSurface::SyncOnly`] agent — which
/// installs nothing on this computer and implements no `HostApp` — offered all
/// five, and each one reached a handler whose only possible answer was
/// `unknown host: claude-code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapabilities {
    pub can_open: bool,
    pub can_verify: bool,
    pub can_repair: bool,
    pub can_open_config: bool,
    pub can_remove: bool,
}

impl HostCapabilities {
    // Why: `can_open` is the host's own answer (some local hosts cannot be
    // launched); the rest follow from whether anything of this agent lives on
    // this computer at all.
    #[must_use]
    pub const fn for_surface(surface: AgentSurface, can_open: bool) -> Self {
        match surface {
            AgentSurface::LocalProfile => Self {
                can_open,
                can_verify: true,
                can_repair: true,
                can_open_config: true,
                can_remove: true,
            },
            // Why: no profile to generate, no config file to open, no process
            // to probe, nothing to remove. Governed entirely from the gateway.
            AgentSurface::SyncOnly => Self {
                can_open: false,
                can_verify: false,
                can_repair: false,
                can_open_config: false,
                can_remove: false,
            },
        }
    }
}

/// The verdict for one agent, plus the three facts the GUI renders around it.
///
/// `is_set_up` means the agent belongs in the Agents list rather than behind
/// "Add agent" (merely "not absent", so partial and stale profiles count);
/// `is_installed` means the managed profile is present and complete;
/// `is_running` is the raw process-table fact — the app is open — and says
/// nothing about whether it is configured, so it is never the headline.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct AgentVerdict {
    pub state: AgentState,
    pub tone: Tone,
    pub reason: AgentReason,
    pub action: Option<AgentAction>,
    pub is_set_up: bool,
    pub is_installed: bool,
    pub is_running: bool,
}

/// Everything `verdict` needs about one host.
#[derive(Debug, Clone, Copy)]
pub struct HostHealthInputs<'a> {
    pub snapshot: Option<&'a crate::integration::HostAppSnapshot>,
    pub proxy: &'a ProxyHealth,
    pub models: HostModelViewRef<'a>,
    pub has_download_url: bool,
    pub surface: AgentSurface,
    pub manifest_synced: bool,
    pub can_open: bool,
}

/// The model-availability facts, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct HostModelViewRef<'a> {
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: &'a [String],
}

impl<'a> From<&'a crate::integration::host_app::HostModelView> for HostModelViewRef<'a> {
    fn from(v: &'a crate::integration::host_app::HostModelView) -> Self {
        Self {
            checked: v.checked,
            available: v.available,
            unconfigured_providers: &v.unconfigured_providers,
        }
    }
}

// Why: precedence is deliberate and matches the order a reader can act on —
// the most specific, most fixable fault wins, so nobody is told "the proxy is
// down" when the real answer is "the app is not installed".
#[must_use]
pub fn verdict(input: &HostHealthInputs<'_>) -> AgentVerdict {
    if input.surface == AgentSurface::SyncOnly {
        return super::sync_only::sync_only_verdict(input.manifest_synced);
    }

    // Why: a host that has never been probed is unknown, not absent. Collapsing
    // those two is what made healthy agents read as broken for the first
    // second after launch.
    let Some(snap) = input.snapshot else {
        return AgentVerdict {
            state: AgentState::Checking,
            tone: AgentState::Checking.tone(),
            reason: AgentReason::NeverProbed,
            action: None,
            is_set_up: false,
            is_installed: false,
            is_running: false,
        };
    };

    let is_set_up = !matches!(snap.profile_state, ProfileState::Absent);
    let is_installed = matches!(snap.profile_state, ProfileState::Installed);
    let is_running = snap.host_running;
    let finish = |state: AgentState, reason, action| AgentVerdict {
        state,
        tone: state.tone(),
        reason,
        action,
        is_set_up,
        is_installed,
        is_running,
    };

    // Why: `Unknown` is not `NotInstalled` — an inconclusive probe must not be
    // rendered as absence.
    if snap.app_installed == AppInstallState::NotInstalled {
        return finish(
            AgentState::Attention,
            AgentReason::AppMissing,
            input.has_download_url.then_some(AgentAction::Download),
        );
    }

    match &snap.profile_state {
        ProfileState::Stale { reason } => {
            return finish(
                AgentState::Attention,
                AgentReason::Stale { cause: *reason },
                Some(AgentAction::Repair),
            );
        },
        ProfileState::Partial { missing_required } => {
            return finish(
                AgentState::Attention,
                AgentReason::Partial {
                    missing: missing_required.join(", "),
                },
                Some(AgentAction::Repair),
            );
        },
        ProfileState::Absent => {
            return finish(
                AgentState::NotSetUp,
                AgentReason::Absent,
                Some(AgentAction::Add),
            );
        },
        ProfileState::Installed => {},
    }

    if input.models.checked && !input.models.available {
        let reason = if input.models.unconfigured_providers.is_empty() {
            AgentReason::NoModels
        } else {
            AgentReason::NoKey {
                providers: input.models.unconfigured_providers.join(", "),
            }
        };
        return finish(AgentState::Attention, reason, None);
    }

    let open = input.can_open.then_some(AgentAction::Open);
    match input.proxy.state {
        ProxyProbeState::Unconfigured => finish(AgentState::Ready, AgentReason::Awaiting, open),
        ProxyProbeState::Listening => finish(
            AgentState::Working,
            AgentReason::Governed {
                when_unix: (snap.probed_at_unix > 0).then_some(snap.probed_at_unix),
            },
            open,
        ),
        // Why: before the first proxy probe lands there is no finding to report.
        ProxyProbeState::Unknown => finish(AgentState::Checking, AgentReason::NeverProbed, None),
        probe => finish(
            AgentState::Down,
            AgentReason::ProxyDown { probe },
            Some(AgentAction::Verify),
        ),
    }
}

pub use super::agent_fleet::{AgentFleetSummary, AgentFleets, FleetHeadline, FleetState};
pub use super::sync_only::{SYNC_ONLY_AGENTS, SyncOnlyAgent, sync_only_agent};
