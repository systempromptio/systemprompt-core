//! In-memory state for the one-time post-link provisioning run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

pub use crate::wire::first_run::{FirstRunPhase, StepStatus};

#[derive(Debug, Clone)]
pub struct FirstRunHost {
    pub host_id: HostKind,
    pub display_name: String,
    pub status: StepStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FirstRunState {
    pub active: bool,
    pub done: bool,
    pub phase: FirstRunPhase,
    pub hosts: Vec<FirstRunHost>,
    pub sync: StepStatus,
    pub error: Option<String>,
    pub started_at_unix: u64,
    /// Outcome of the run's first sync, held until every host is terminal.
    pub sync_result: Option<bool>,
    /// A host finished installing after the sync started, so the sync may
    /// have missed it and one follow-up sync is owed.
    pub host_installed_during_sync: bool,
    pub followup_sync: bool,
}

impl FirstRunState {
    pub fn host_mut(&mut self, host_id: HostKind) -> Option<&mut FirstRunHost> {
        self.hosts.iter_mut().find(|h| h.host_id == host_id)
    }

    pub fn host(&self, host_id: HostKind) -> Option<&FirstRunHost> {
        self.hosts.iter().find(|h| h.host_id == host_id)
    }

    pub fn all_hosts_terminal(&self) -> bool {
        self.hosts.iter().all(|h| h.status.is_terminal())
    }

    pub fn ready_to_finish(&self) -> bool {
        self.sync_result.is_some() && !self.followup_sync && self.all_hosts_terminal()
    }

    pub fn any_host_installed(&self) -> bool {
        self.hosts.iter().any(|h| h.status == StepStatus::Done)
    }
}
