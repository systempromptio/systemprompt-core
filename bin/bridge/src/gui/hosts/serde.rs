//! Builds the host-app wire payloads from the GUI state snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::state::AppStateSnapshot;
use crate::integration::agent_health::{
    AgentFleets, AgentSurface, HostCapabilities, HostHealthInputs, HostModelViewRef,
    SYNC_ONLY_AGENTS,
};
use crate::integration::host_app::{ConfigFormat, HostKind};
pub(crate) use crate::wire::hosts::{
    HostEntryPayload, HostHealthPayload, HostsPayload, ProxyPayload,
};

fn build_entry<'a>(
    snap: &'a AppStateSnapshot,
    host: &'static dyn crate::integration::HostApp,
) -> HostEntryPayload<'a> {
    let st = snap.hosts.get(host.id());
    let effective = crate::gateway::model_view::effective_surfaces(
        host.id(),
        host.accepted_surfaces(),
        &snap.host_model_protocols,
    );
    let overridden =
        crate::gateway::model_view::has_surface_override(host.id(), &snap.host_model_protocols);
    let view = crate::gateway::model_view::host_model_view(&snap.provider_health, &effective);
    let snapshot = st.and_then(|s| s.snapshot.as_ref());
    let verdict = crate::integration::agent_health::verdict(&HostHealthInputs {
        snapshot,
        proxy: &snap.hosts.local_proxy,
        models: HostModelViewRef::from(&view),
        has_download_url: !host.download_url().is_empty(),
        surface: AgentSurface::LocalProfile,
        manifest_synced: snap.manifest_synced(),
        can_open: host.can_open(),
    });
    let caps = HostCapabilities::for_surface(AgentSurface::LocalProfile, host.can_open());
    HostEntryPayload {
        id: host.id(),
        display_name: host.display_name(),
        kind: host.kind(),
        description: host.description(),
        icon: host.icon_id(),
        config_format: host.config_format(),
        download_url: host.download_url(),
        install_action_label: host.install_action_label(),
        can_open: caps.can_open,
        can_verify: caps.can_verify,
        can_repair: caps.can_repair,
        can_open_config: caps.can_open_config,
        can_remove: caps.can_remove,
        probe_in_flight: st.is_some_and(|s| s.probe_in_flight),
        enabled: snap.enabled_hosts.iter().any(|h| h == host.id()),
        last_generated_profile: st.and_then(|s| s.last_generated_profile.as_ref()),
        health: snapshot.map(HostHealthPayload::from),
        compatible_models: view.compatible_models,
        models_checked: view.checked,
        compatible_models_available: view.available,
        unconfigured_providers: view.unconfigured_providers,
        model_protocols: effective.iter().map(|s| s.as_tag().to_owned()).collect(),
        model_protocols_overridden: overridden,
        surface: AgentSurface::LocalProfile,
        verdict,
    }
}

// Why: these hosts are enabled by the same manifest as the desktop ones but
// have no `HostApp`, so `host_apps()` never yields them and they used to be
// invisible — including `claude-code`, which is what most readers are running
// while they look at this screen.
fn build_sync_only_entry<'a>(
    snap: &'a AppStateSnapshot,
    agent: &'a crate::integration::agent_health::SyncOnlyAgent,
) -> HostEntryPayload<'a> {
    let verdict = crate::integration::agent_health::verdict(&HostHealthInputs {
        snapshot: None,
        proxy: &snap.hosts.local_proxy,
        models: HostModelViewRef {
            checked: false,
            available: false,
            unconfigured_providers: &[],
        },
        has_download_url: false,
        surface: AgentSurface::SyncOnly,
        manifest_synced: snap.manifest_synced(),
        can_open: false,
    });
    let caps = HostCapabilities::for_surface(AgentSurface::SyncOnly, false);
    HostEntryPayload {
        id: agent.id,
        display_name: agent.display_name,
        kind: HostKind::CliTool,
        description: agent.description,
        icon: agent.icon,
        config_format: ConfigFormat::Json,
        download_url: "",
        install_action_label: "",
        can_open: caps.can_open,
        can_verify: caps.can_verify,
        can_repair: caps.can_repair,
        can_open_config: caps.can_open_config,
        can_remove: caps.can_remove,
        probe_in_flight: false,
        enabled: snap.enabled_hosts.iter().any(|h| h == agent.id),
        last_generated_profile: None,
        health: None,
        compatible_models: Vec::new(),
        models_checked: false,
        compatible_models_available: false,
        unconfigured_providers: Vec::new(),
        model_protocols: Vec::new(),
        model_protocols_overridden: false,
        surface: AgentSurface::SyncOnly,
        verdict,
    }
}

pub(crate) fn single_host_payload<'a>(
    snap: &'a AppStateSnapshot,
    host_id: &str,
) -> Option<HostEntryPayload<'a>> {
    // Why: the sync-only fallback is not decoration. `emit_host_changed` sends
    // whatever this returns on the `host.changed` channel, so a `None` here
    // published a null body that the front end silently dropped — the row for
    // a sync-only agent could never be updated after its first full snapshot.
    crate::integration::host_apps()
        .iter()
        .copied()
        .find(|h| h.id() == host_id)
        .map(|host| build_entry(snap, host))
        .or_else(|| {
            crate::integration::sync_only_agent(host_id)
                .map(|agent| build_sync_only_entry(snap, agent))
        })
}

pub(crate) fn payload(snap: &AppStateSnapshot) -> HostsPayload<'_> {
    // Why: the last-sync manifest is the instance's host gate. Before the
    // first sync it is empty and every registered host stays visible;
    // afterwards hosts the instance disabled are dropped from the GUI.
    let mut entries: Vec<HostEntryPayload<'_>> = crate::integration::host_apps()
        .iter()
        .copied()
        .map(|host| build_entry(snap, host))
        .collect();
    entries.extend(
        SYNC_ONLY_AGENTS
            .iter()
            .map(|agent| build_sync_only_entry(snap, agent)),
    );
    if !snap.enabled_hosts.is_empty() {
        entries.retain(|e| e.enabled);
    }
    let agent_fleet = AgentFleets::fold(
        &entries
            .iter()
            .map(|e| e.verdict.clone())
            .collect::<Vec<_>>(),
    );
    HostsPayload {
        host_apps: entries,
        hosts_gated: snap.manifest_synced(),
        agent_fleet,
        local_proxy: ProxyPayload::from(&snap.hosts.local_proxy),
        agents_onboarded: snap.agents_onboarded,
        first_run: crate::gui::first_run::serde::build(&snap.first_run),
    }
}
