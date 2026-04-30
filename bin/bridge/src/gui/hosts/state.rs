use std::collections::HashMap;

use crate::integration::{HostAppSnapshot, ProxyHealth};

#[derive(Debug, Clone, Default)]
pub struct HostState {
    pub snapshot: Option<HostAppSnapshot>,
    pub probe_in_flight: bool,
    pub last_generated_profile: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HostsState {
    pub by_id: HashMap<String, HostState>,
    pub local_proxy: ProxyHealth,
    pub proxy_probe_in_flight: bool,
}

impl HostsState {
    pub fn entry(&mut self, host_id: &str) -> &mut HostState {
        self.by_id.entry(host_id.to_string()).or_default()
    }

    pub fn get(&self, host_id: &str) -> Option<&HostState> {
        self.by_id.get(host_id)
    }
}
