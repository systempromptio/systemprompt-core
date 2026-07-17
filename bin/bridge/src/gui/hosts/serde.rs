//! JSON payload shapes for host-app status sent to the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::gui::state::AppStateSnapshot;
use crate::integration::host_app::{ConfigFormat, HostKind};
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};

#[derive(Serialize)]
pub(crate) struct HostsPayload<'a> {
    pub host_apps: Vec<HostEntryPayload<'a>>,
    pub local_proxy: &'a ProxyHealth,
    pub agents_onboarded: bool,
}

#[derive(Serialize)]
pub(crate) struct HostEntryPayload<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
    pub kind: HostKind,
    pub description: &'a str,
    pub icon: &'a str,
    pub config_format: ConfigFormat,
    pub download_url: &'a str,
    pub install_action_label: &'a str,
    pub probe_in_flight: bool,
    pub enabled: bool,
    pub last_generated_profile: Option<&'a GeneratedProfile>,
    pub snapshot: Option<&'a HostAppSnapshot>,
    pub compatible_models: Vec<String>,
    pub models_checked: bool,
    pub compatible_models_available: bool,
    pub unconfigured_providers: Vec<String>,
    /// Wire-protocol filter in force; empty means "all models".
    pub model_protocols: Vec<String>,
    pub model_protocols_overridden: bool,
}

fn build_entry<'a>(
    snap: &'a AppStateSnapshot,
    host: &'static dyn crate::integration::HostApp,
) -> HostEntryPayload<'a> {
    let st = snap.hosts.get(host.id());
    let effective = crate::integration::host_app::effective_surfaces(
        host.id(),
        host.accepted_surfaces(),
        &snap.host_model_protocols,
    );
    let overridden =
        crate::integration::host_app::has_surface_override(host.id(), &snap.host_model_protocols);
    let view = crate::integration::host_app::host_model_view(&snap.provider_health, &effective);
    HostEntryPayload {
        id: host.id(),
        display_name: host.display_name(),
        kind: host.kind(),
        description: host.description(),
        icon: host.icon_id(),
        config_format: host.config_format(),
        download_url: host.download_url(),
        install_action_label: host.install_action_label(),
        probe_in_flight: st.is_some_and(|s| s.probe_in_flight),
        enabled: snap.enabled_hosts.iter().any(|h| h == host.id()),
        last_generated_profile: st.and_then(|s| s.last_generated_profile.as_ref()),
        snapshot: st.and_then(|s| s.snapshot.as_ref()),
        compatible_models: view.compatible_models,
        models_checked: view.checked,
        compatible_models_available: view.available,
        unconfigured_providers: view.unconfigured_providers,
        model_protocols: effective.iter().map(|s| s.as_tag().to_owned()).collect(),
        model_protocols_overridden: overridden,
    }
}

pub(crate) fn single_host_payload<'a>(
    snap: &'a AppStateSnapshot,
    host_id: &str,
) -> Option<HostEntryPayload<'a>> {
    crate::integration::host_apps()
        .iter()
        .copied()
        .find(|h| h.id() == host_id)
        .map(|host| build_entry(snap, host))
}

pub(crate) fn payload(snap: &AppStateSnapshot) -> HostsPayload<'_> {
    let entries = crate::integration::host_apps()
        .iter()
        .copied()
        .map(|host| build_entry(snap, host))
        .collect();
    HostsPayload {
        host_apps: entries,
        local_proxy: &snap.hosts.local_proxy,
        agents_onboarded: snap.agents_onboarded,
    }
}
