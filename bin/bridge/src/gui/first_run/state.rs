//! In-memory state for the one-time post-link provisioning run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use crate::wire::first_run::{FirstRunPhase, StepStatus};

#[derive(Debug, Clone)]
pub struct FirstRunHost {
    pub host_id: String,
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
