//! Per-host probe state held by the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};

/// Monotonic per-host probe ticket: a result is applied only when its ticket
/// is the newest issued for that host, so overlapping probes cannot land out
/// of order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ProbeSeq(u64);

impl ProbeSeq {
    const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostState {
    pub snapshot: Option<HostAppSnapshot>,
    pub probe_in_flight: bool,
    pub probe_seq: ProbeSeq,
    pub last_generated_profile: Option<GeneratedProfile>,
}

impl HostState {
    pub const fn issue_probe(&mut self) -> ProbeSeq {
        self.probe_seq = self.probe_seq.next();
        self.probe_in_flight = true;
        self.probe_seq
    }

    pub fn is_newest(&self, seq: ProbeSeq) -> bool {
        self.probe_seq == seq
    }
}

#[derive(Debug, Clone, Default)]
pub struct HostsState {
    pub by_id: HashMap<String, HostState>,
    pub local_proxy: ProxyHealth,
    pub proxy_probe_in_flight: bool,
}

impl HostsState {
    pub fn entry(&mut self, host_id: &str) -> &mut HostState {
        self.by_id.entry(host_id.to_owned()).or_default()
    }

    pub fn get(&self, host_id: &str) -> Option<&HostState> {
        self.by_id.get(host_id)
    }
}
