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
use crate::integration::proxy_probe::{ProxyHealth, ProxyProbeState};

/// What the reader is told about one agent.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentState {
    /// Governed and proven: profile installed, app present, proxy answering.
    Working,
    /// Installed and correct, but never launched.
    Ready,
    /// A specific, fixable local fault.
    Attention,
    /// No profile here. A thing you may add, not a thing that is broken.
    NotSetUp,
    /// Should work, but the local proxy is not answering.
    Down,
    /// Never probed, or a probe is in flight. NOT evidence of absence.
    Checking,
}

/// Why the agent is in that state. Carries the arguments its message needs.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum AgentReason {
    Governed {
        when_unix: Option<u64>,
    },
    Awaiting,
    AppMissing,
    Stale {
        cause: StaleReason,
    },
    Partial {
        missing: String,
    },
    Absent,
    NoKey {
        providers: String,
    },
    NoModels,
    ProxyDown {
        probe: ProxyProbeState,
    },
    NeverProbed,
    /// Routed through the gateway centrally; nothing to install on this
    /// machine.
    CloudManaged,
}

/// The one button that fixes this state, if there is one.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum AgentAction {
    Download,
    Repair,
    Verify,
    Open,
    Add,
}

/// Whether this agent is configured on this machine at all.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSurface {
    /// Has a managed profile this bridge installs and probes locally.
    LocalProfile,
    /// Gateway-enabled but has no local profile — it syncs, it does not
    /// install.
    SyncOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentVerdict {
    pub state: AgentState,
    pub reason: AgentReason,
    pub action: Option<AgentAction>,
    /// Belongs in the Agents list rather than behind "Add agent".
    pub is_set_up: bool,
    /// The managed profile is present and complete. Distinct from `is_set_up`,
    /// which is merely "not absent" and so includes partial and stale profiles.
    pub is_installed: bool,
    /// Raw process-table fact. Named for what it is: the app is open. It says
    /// nothing about whether the app is configured, so it is never the
    /// headline.
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
    /// Whether the instance has synced its manifest at least once.
    pub manifest_synced: bool,
}

/// The model-availability facts, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct HostModelViewRef<'a> {
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: &'a [String],
}

// `HostModelView` only exists where a GUI is built.
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl<'a> From<&'a crate::integration::host_app::HostModelView> for HostModelViewRef<'a> {
    fn from(v: &'a crate::integration::host_app::HostModelView) -> Self {
        Self {
            checked: v.checked,
            available: v.available,
            unconfigured_providers: &v.unconfigured_providers,
        }
    }
}

/// Decide one agent's state.
///
/// Precedence is deliberate and matches the order a reader can act on: the most
/// specific, most fixable fault wins, so nobody is told "the proxy is down"
/// when the real answer is "the app is not installed".
#[must_use]
pub fn verdict(input: &HostHealthInputs<'_>) -> AgentVerdict {
    if input.surface == AgentSurface::SyncOnly {
        return sync_only_verdict(input.manifest_synced);
    }

    // A host that has never been probed is unknown, not absent. Collapsing
    // those two is what made healthy agents read as broken for the first
    // second after launch.
    let Some(snap) = input.snapshot else {
        return AgentVerdict {
            state: AgentState::Checking,
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
    let finish = |state, reason, action| AgentVerdict {
        state,
        reason,
        action,
        is_set_up,
        is_installed,
        is_running,
    };

    // `Unknown` is not `NotInstalled` — an inconclusive probe must not be
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

    match input.proxy.state {
        ProxyProbeState::Unconfigured => finish(
            AgentState::Ready,
            AgentReason::Awaiting,
            Some(AgentAction::Open),
        ),
        ProxyProbeState::Listening => finish(
            AgentState::Working,
            AgentReason::Governed {
                when_unix: (snap.probed_at_unix > 0).then_some(snap.probed_at_unix),
            },
            Some(AgentAction::Open),
        ),
        // Before the first proxy probe lands there is no finding to report.
        ProxyProbeState::Unknown => finish(AgentState::Checking, AgentReason::NeverProbed, None),
        probe => finish(
            AgentState::Down,
            AgentReason::ProxyDown { probe },
            Some(AgentAction::Verify),
        ),
    }
}

/// A sync-only agent is governed by construction — it reaches the gateway
/// directly — so the only thing this machine can say about it is whether the
/// manifest that enables it has arrived yet.
const fn sync_only_verdict(manifest_synced: bool) -> AgentVerdict {
    if manifest_synced {
        AgentVerdict {
            state: AgentState::Working,
            reason: AgentReason::CloudManaged,
            action: None,
            is_set_up: true,
            is_installed: true,
            is_running: false,
        }
    } else {
        AgentVerdict {
            state: AgentState::Checking,
            reason: AgentReason::NeverProbed,
            action: None,
            is_set_up: false,
            is_installed: false,
            is_running: false,
        }
    }
}

/// How the fleet as a whole reads.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FleetState {
    Ok,
    Warn,
    Err,
    #[default]
    Unknown,
}

/// The one-line summary code for the fleet — also the FTL key suffix.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FleetHeadline {
    AllWorking,
    NeedsAttention,
    NotWorking,
    Checking,
    #[default]
    NoneEnabled,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct AgentFleetSummary {
    pub total: usize,
    pub working: usize,
    pub ready: usize,
    pub attention: usize,
    pub not_set_up: usize,
    pub down: usize,
    pub checking: usize,
    /// How many agent apps are open. A footer fact, never the headline.
    pub running: usize,
    pub state: FleetState,
    pub headline: FleetHeadline,
}

impl AgentFleetSummary {
    /// Fold per-agent verdicts into the fleet summary.
    ///
    /// Takes only verdicts — never the raw snapshot — so the card and the rows
    /// are structurally incapable of reaching different conclusions.
    #[must_use]
    pub fn fold<'a>(verdicts: impl Iterator<Item = &'a AgentVerdict>) -> Self {
        let mut s = Self {
            state: FleetState::Unknown,
            headline: FleetHeadline::NoneEnabled,
            ..Self::default()
        };
        for v in verdicts {
            s.total += 1;
            if v.is_running {
                s.running += 1;
            }
            match v.state {
                AgentState::Working => s.working += 1,
                AgentState::Ready => s.ready += 1,
                AgentState::Attention => s.attention += 1,
                AgentState::NotSetUp => s.not_set_up += 1,
                AgentState::Down => s.down += 1,
                AgentState::Checking => s.checking += 1,
            }
        }

        s.state = if s.total == 0 {
            FleetState::Unknown
        } else if s.down > 0 {
            FleetState::Err
        } else if s.attention > 0 || s.not_set_up > 0 {
            FleetState::Warn
        } else if s.checking == s.total {
            FleetState::Unknown
        } else {
            FleetState::Ok
        };

        s.headline = match s.state {
            FleetState::Ok => FleetHeadline::AllWorking,
            FleetState::Warn => FleetHeadline::NeedsAttention,
            FleetState::Err => FleetHeadline::NotWorking,
            FleetState::Unknown if s.total == 0 => FleetHeadline::NoneEnabled,
            FleetState::Unknown => FleetHeadline::Checking,
        };
        s
    }
}

/// Both scopes the GUI needs: every enabled agent, and only those set up here.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct AgentFleets {
    pub all: AgentFleetSummary,
    pub set_up: AgentFleetSummary,
}

impl AgentFleets {
    #[must_use]
    pub fn fold(verdicts: &[AgentVerdict]) -> Self {
        Self {
            all: AgentFleetSummary::fold(verdicts.iter()),
            set_up: AgentFleetSummary::fold(verdicts.iter().filter(|v| v.is_set_up)),
        }
    }
}

/// An agent the gateway governs centrally, with nothing to install locally.
///
/// `claude-code` and `cowork` are enabled in the instance manifest exactly like
/// the desktop hosts, but they have no [`crate::integration::HostApp`] — they
/// reach the gateway themselves and only receive skill/plugin sync from here.
/// Before this table they were simply invisible: a user running Claude Code
/// looked at the Agents card and saw no sign of the agent they were using.
#[derive(Debug, Clone, Copy)]
pub struct SyncOnlyAgent {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
}

/// Gateway host ids that sync but never install a local profile.
///
/// Kept in step with `KNOWN_HOSTS` in the gateway's bridge route: every id
/// there that has no `HostApp` implementation belongs here, or it silently
/// disappears from the GUI.
pub const SYNC_ONLY_AGENTS: &[SyncOnlyAgent] = &[
    SyncOnlyAgent {
        id: "claude-code",
        display_name: "Claude Code",
        description: "Governed through the gateway; skills and plugins sync from here.",
        icon: "claude-code",
    },
    SyncOnlyAgent {
        id: "cowork",
        display_name: "Cowork",
        description: "Governed through the gateway; artifacts and plugins sync from here.",
        icon: "cowork",
    },
];

/// The sync-only agent for a host id, if that id is one.
#[must_use]
pub fn sync_only_agent(host_id: &str) -> Option<&'static SyncOnlyAgent> {
    SYNC_ONLY_AGENTS.iter().find(|a| a.id == host_id)
}
