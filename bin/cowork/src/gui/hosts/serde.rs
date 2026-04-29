use serde::Serialize;

use crate::gui::state::AppStateSnapshot;
use crate::integration::{HostAppSnapshot, ProxyHealth};

#[derive(Serialize)]
pub(crate) struct HostsPayload<'a> {
    pub host_apps: Vec<HostEntryPayload<'a>>,
    pub local_proxy: &'a ProxyHealth,
}

#[derive(Serialize)]
pub(crate) struct HostEntryPayload<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
    pub install_action_label: &'a str,
    pub probe_in_flight: bool,
    pub last_generated_profile: Option<&'a str>,
    pub snapshot: Option<&'a HostAppSnapshot>,
}

pub(crate) fn payload(snap: &AppStateSnapshot) -> HostsPayload<'_> {
    let mut entries = Vec::new();
    for host in crate::integration::host_apps() {
        let st = snap.hosts.get(host.id());
        entries.push(HostEntryPayload {
            id: host.id(),
            display_name: host.display_name(),
            install_action_label: host.install_action_label(),
            probe_in_flight: st.map(|s| s.probe_in_flight).unwrap_or(false),
            last_generated_profile: st.and_then(|s| s.last_generated_profile.as_deref()),
            snapshot: st.and_then(|s| s.snapshot.as_ref()),
        });
    }
    HostsPayload {
        host_apps: entries,
        local_proxy: &snap.hosts.local_proxy,
    }
}
