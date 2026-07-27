//! In-memory state for the one-time post-link provisioning run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// How far the run has got. `Complete` and `Failed` are both terminal; the
/// difference is only what the wizard says, not whether the user may leave.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FirstRunPhase {
    #[default]
    Idle,
    Probing,
    Installing,
    Syncing,
    Complete,
    Failed,
}

impl FirstRunPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Probing => "probing",
            Self::Installing => "installing",
            Self::Syncing => "syncing",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

/// Where one host has got to. `Skipped` means the host app is not installed on
/// this machine, which is not a failure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StepStatus {
    #[default]
    Pending,
    Probing,
    Generating,
    Installing,
    Done,
    Failed,
    Skipped,
}

impl StepStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Probing => "probing",
            Self::Generating => "generating",
            Self::Installing => "installing",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    /// Terminal statuses take no further action. The re-probe that
    /// `on_profile_install_finished` fires would otherwise re-enter the
    /// orchestrator and restart the host's chain forever.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Skipped)
    }
}

#[derive(Debug, Clone)]
pub struct FirstRunHost {
    pub host_id: String,
    pub display_name: String,
    pub status: StepStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FirstRunState {
    /// A run is in flight. While true the wizard refuses to finish.
    pub active: bool,
    /// A run has completed at some point on this machine (sentinel present).
    pub done: bool,
    pub phase: FirstRunPhase,
    pub hosts: Vec<FirstRunHost>,
    pub sync: StepStatus,
    pub error: Option<String>,
    /// When the run began, for the watchdog. Zero when no run has started.
    pub started_at_unix: u64,
}

impl FirstRunState {
    pub fn host_mut(&mut self, host_id: &str) -> Option<&mut FirstRunHost> {
        self.hosts.iter_mut().find(|h| h.host_id == host_id)
    }

    pub fn host(&self, host_id: &str) -> Option<&FirstRunHost> {
        self.hosts.iter().find(|h| h.host_id == host_id)
    }

    pub fn all_hosts_terminal(&self) -> bool {
        self.hosts.iter().all(|h| h.status.is_terminal())
    }

    pub fn any_host_installed(&self) -> bool {
        self.hosts.iter().any(|h| h.status == StepStatus::Done)
    }
}
