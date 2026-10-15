//! The inputs an agent-health verdict is computed from: the host's surface,
//! the actions that surface offers, and the probe and sync facts handed to
//! [`super::agent_health::verdict`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::proxy_probe::ProxyHealth;

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

/// Actions available for a host surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapabilities {
    pub can_open: bool,
    pub can_verify: bool,
    pub can_repair: bool,
    pub can_open_config: bool,
    pub can_remove: bool,
}

impl HostCapabilities {
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

/// A sync step for this host that failed and that the live probe does not
/// observe (skills, connectors, plugins), so the last attempt is the only
/// evidence of it.
#[derive(Debug, Clone)]
pub struct ApplyFailure {
    pub emitter: String,
    pub error: String,
    pub needs_elevation: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct HostHealthInputs<'a> {
    pub snapshot: Option<&'a crate::integration::HostAppSnapshot>,
    pub apply_failures: &'a [ApplyFailure],
    pub proxy: &'a ProxyHealth,
    pub models: HostModelViewRef<'a>,
    pub has_download_url: bool,
    pub surface: AgentSurface,
    pub manifest_synced: bool,
    pub gateway_routed: bool,
    pub can_open: bool,
    pub pending: PendingStep,
}

/// A step the bridge started that only the user or the OS can finish.
///
/// `OsApproval` is a macOS configuration profile handed to System Settings at
/// `offered_at_unix` that has not been approved yet, so the probe still reads
/// no profile; macOS discards an offered profile after
/// [`OS_APPROVAL_WINDOW_SECS`], so it must be offered again after that;
/// `Restart` is a profile written while the app was open, which the app reads
/// only on its next launch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PendingStep {
    #[default]
    None,
    OsApproval {
        offered_at_unix: u64,
    },
    Restart,
}

/// How long macOS keeps a downloaded `.mobileconfig` waiting in Device
/// Management before it silently drops it.
pub const OS_APPROVAL_WINDOW_SECS: u64 = 8 * 60;

impl PendingStep {
    /// The offer has outlived the window macOS keeps it for, judged at `now`.
    #[must_use]
    pub const fn approval_expired(self, now_unix: u64) -> bool {
        match self {
            Self::OsApproval { offered_at_unix } => {
                now_unix >= offered_at_unix.saturating_add(OS_APPROVAL_WINDOW_SECS)
            },
            Self::None | Self::Restart => false,
        }
    }

    /// How a probe moves the step: an approved macOS profile leaves only the
    /// restart (if the app is open), and a restart is done once the app has
    /// been seen closed. A probe that could not read the process table keeps
    /// the restart owed.
    #[must_use]
    pub fn after_probe(self, snap: &crate::integration::HostAppSnapshot) -> Self {
        let running = snap.host_running == Some(true);
        match self {
            Self::OsApproval { .. }
                if matches!(snap.profile_state, crate::integration::ProfileState::Absent) =>
            {
                self
            },
            Self::OsApproval { .. } | Self::Restart if running => Self::Restart,
            Self::Restart if snap.host_running.is_none() => self,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostModelViewRef<'a> {
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: &'a [String],
    pub compatible_models: &'a [String],
}

impl<'a> From<&'a crate::gateway::model_view::HostModelView> for HostModelViewRef<'a> {
    fn from(v: &'a crate::gateway::model_view::HostModelView) -> Self {
        Self {
            checked: v.checked,
            available: v.available,
            unconfigured_providers: &v.unconfigured_providers,
            compatible_models: &v.compatible_models,
        }
    }
}
