//! First-run wizard transitions held in application state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppState, now_unix};

impl AppState {
    pub fn first_run_active(&self) -> bool {
        self.inner.read().first_run.active
    }

    pub fn begin_first_run(&self, hosts: &[(String, String)]) {
        use crate::gui::first_run::state::{FirstRunHost, FirstRunPhase, StepStatus};
        let mut guard = self.inner.write();
        guard.first_run.active = true;
        guard.first_run.phase = FirstRunPhase::Probing;
        guard.first_run.sync = StepStatus::Pending;
        guard.first_run.error = None;
        guard.first_run.started_at_unix = now_unix();
        guard.first_run.hosts = hosts
            .iter()
            .map(|(id, name)| FirstRunHost {
                host_id: id.clone(),
                display_name: name.clone(),
                status: StepStatus::Probing,
                error: None,
            })
            .collect();
    }

    pub fn set_first_run_host(
        &self,
        host_id: &str,
        status: crate::gui::first_run::state::StepStatus,
        error: Option<String>,
    ) {
        let mut guard = self.inner.write();
        if let Some(host) = guard.first_run.host_mut(host_id) {
            host.status = status;
            host.error = error;
        }
    }

    pub fn set_first_run_phase(&self, phase: crate::gui::first_run::state::FirstRunPhase) {
        self.inner.write().first_run.phase = phase;
    }

    pub fn set_first_run_sync(&self, status: crate::gui::first_run::state::StepStatus) {
        self.inner.write().first_run.sync = status;
    }

    pub fn finish_first_run(&self, phase: crate::gui::first_run::state::FirstRunPhase) {
        let mut guard = self.inner.write();
        guard.first_run.phase = phase;
        guard.first_run.active = false;
        guard.first_run.done = true;
    }
}
