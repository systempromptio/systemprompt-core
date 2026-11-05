//! Host and local-proxy probe ledger held in application state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

use super::AppState;
use crate::gui::hosts::state::ProbeSeq;
use crate::integration::agent_health::PendingStep;
use crate::integration::{HostAppSnapshot, ProxyHealth};

const RESTART_PROMPT_LIFETIME: std::time::Duration = std::time::Duration::from_mins(10);

impl AppState {
    pub fn apply_host_snapshot(
        &self,
        host_id: HostKind,
        seq: ProbeSeq,
        snap: HostAppSnapshot,
    ) -> bool {
        let mut guard = self.snap_mut();
        let entry = guard.hosts.entry(host_id);
        if !entry.is_newest(seq) {
            return false;
        }
        entry.pending = entry.pending.after_probe(&snap);
        // Why: a relaunch between two probes is never seen as closed, so a
        // prompt that cannot tell it was obeyed does not stay for good.
        if entry.pending == PendingStep::Restart {
            let since = *entry
                .restart_owed_since
                .get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= RESTART_PROMPT_LIFETIME {
                entry.pending = PendingStep::None;
                entry.restart_owed_since = None;
            }
        } else {
            entry.restart_owed_since = None;
        }
        entry.snapshot = Some(snap);
        entry.probe_in_flight = false;
        drop(guard);
        true
    }

    pub fn finish_failed_probe(&self, host_id: Option<(HostKind, ProbeSeq)>) {
        let mut guard = self.snap_mut();
        match host_id {
            Some((id, seq)) => {
                let entry = guard.hosts.entry(id);
                if entry.is_newest(seq) {
                    entry.probe_in_flight = false;
                }
            },
            None => guard.hosts.proxy_probe_in_flight = false,
        }
    }

    pub fn begin_host_probe(&self, host_id: HostKind, exclusive: bool) -> Option<ProbeSeq> {
        let mut guard = self.snap_mut();
        let entry = guard.hosts.entry(host_id);
        if exclusive && entry.probe_in_flight {
            return None;
        }
        let seq = entry.issue_probe();
        drop(guard);
        Some(seq)
    }

    pub fn set_last_generated_profile(
        &self,
        host_id: HostKind,
        profile: crate::integration::GeneratedProfile,
    ) {
        let mut guard = self.snap_mut();
        guard.hosts.entry(host_id).last_generated_profile = Some(profile);
    }

    pub fn set_host_pending(&self, host_id: HostKind, step: PendingStep) {
        self.snap_mut().hosts.entry(host_id).pending = step;
    }

    pub fn mark_proxy_probing(&self) -> bool {
        let mut guard = self.snap_mut();
        if guard.hosts.proxy_probe_in_flight {
            return false;
        }
        guard.hosts.proxy_probe_in_flight = true;
        true
    }

    pub fn apply_proxy_health(&self, health: ProxyHealth) {
        let mut guard = self.snap_mut();
        guard.hosts.local_proxy = health;
        guard.hosts.proxy_probe_in_flight = false;
    }
}
