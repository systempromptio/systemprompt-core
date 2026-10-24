//! Builds the host-app wire payloads from the GUI state snapshot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

use crate::gui::state::AppStateSnapshot;
use crate::integration::agent_health::{
    AgentFleets, AgentSurface, AgentVerdict, ApplyFailure, HostCapabilities, HostHealthInputs,
    HostModelViewRef, PendingStep, SYNC_ONLY_AGENTS, SyncOnlyAgent,
};
use crate::integration::host_app::ConfigFormat;
pub(crate) use crate::wire::hosts::{
    HostEntryPayload, HostHealthPayload, HostsPayload, ProxyPayload,
};

fn apply_failures(
    snap: &AppStateSnapshot,
    host_id: HostKind,
    observed_by: Option<&'static dyn crate::integration::HostApp>,
) -> Vec<ApplyFailure> {
    snap.last_sync_report
        .iter()
        .flat_map(|report| report.host_failures.iter())
        .filter(|f| f.host_id == host_id)
        .filter(|f| {
            !observed_by.is_some_and(|host| host.probe_observes(&f.emitter, f.needs_elevation))
        })
        .map(|f| ApplyFailure {
            emitter: f.emitter.clone(),
            error: f.error.clone(),
            needs_elevation: f.needs_elevation,
        })
        .collect()
}

fn host_verdict(
    snap: &AppStateSnapshot,
    host: &'static dyn crate::integration::HostApp,
    view: &crate::gateway::model_view::HostModelView,
) -> AgentVerdict {
    let failures = apply_failures(snap, host.id(), Some(host));
    let pending = match snap.hosts.get(host.id()).map(|s| s.pending) {
        Some(PendingStep::Restart) if !host.can_quit() => PendingStep::None,
        Some(step) => step,
        None => PendingStep::None,
    };
    let cowork_ambiguous = host.id() == HostKind::ClaudeDesktop
        && !failures.is_empty()
        && matches!(
            crate::integration::cowork_plugins::resolve_target(),
            Err(crate::integration::cowork_plugins::ResolveTargetError::Ambiguous { .. })
        );
    let verdict = crate::integration::agent_health::verdict(&HostHealthInputs {
        snapshot: snap.hosts.get(host.id()).and_then(|s| s.snapshot.as_ref()),
        apply_failures: &failures,
        proxy: &snap.hosts.local_proxy,
        models: HostModelViewRef::from(view),
        has_download_url: !host.download_url().is_empty(),
        surface: AgentSurface::LocalProfile,
        manifest_synced: snap.manifest_synced(),
        gateway_routed: true,
        can_open: host.can_open(),
        pending,
    });
    if cowork_ambiguous && verdict.is_set_up {
        return verdict.with_cowork_session_ambiguous();
    }
    verdict
}

fn sync_only_verdict(snap: &AppStateSnapshot, agent: &SyncOnlyAgent) -> AgentVerdict {
    let failures = apply_failures(snap, agent.id, None);
    crate::integration::agent_health::verdict(&HostHealthInputs {
        snapshot: None,
        apply_failures: &failures,
        proxy: &snap.hosts.local_proxy,
        models: HostModelViewRef {
            checked: false,
            available: false,
            unconfigured_providers: &[],
            missing_personal_keys: &[],
            compatible_models: &[],
        },
        has_download_url: false,
        surface: AgentSurface::SyncOnly,
        manifest_synced: snap.manifest_synced(),
        gateway_routed: crate::integration::sync_only::gateway_routed(agent),
        can_open: false,
        pending: PendingStep::None,
    })
}

fn model_view(
    snap: &AppStateSnapshot,
    host: &'static dyn crate::integration::HostApp,
) -> (
    crate::gateway::model_view::HostClients,
    crate::gateway::model_view::HostModelView,
) {
    let clients =
        crate::gateway::model_view::HostClients::for_host(host.id(), &snap.host_model_protocols);
    let view = crate::gateway::model_view::host_model_view(
        &snap.inference_catalog.providers,
        &clients.families,
    );
    (clients, view)
}

fn cowork_session(id: HostKind) -> Option<crate::wire::hosts::CoworkSessionPayload> {
    if id != HostKind::ClaudeDesktop {
        return None;
    }
    let (target, source) = crate::integration::cowork_plugins::resolve_target_with_source()
        .ok()
        .flatten()?;
    let path = target.session_org_dir;
    Some(crate::wire::hosts::CoworkSessionPayload {
        org: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.display().to_string(),
        source: source.as_str(),
    })
}

fn is_enabled(snap: &AppStateSnapshot, id: HostKind) -> bool {
    snap.enabled_hosts.contains(&id)
}

pub fn host_verdicts(snap: &AppStateSnapshot) -> Vec<(HostKind, AgentVerdict)> {
    let mut out: Vec<(HostKind, AgentVerdict)> = crate::integration::host_apps()
        .iter()
        .copied()
        .filter(|host| is_enabled(snap, host.id()))
        .map(|host| {
            (
                host.id(),
                host_verdict(snap, host, &model_view(snap, host).1),
            )
        })
        .collect();
    out.extend(
        SYNC_ONLY_AGENTS
            .iter()
            .filter(|agent| is_enabled(snap, agent.id))
            .map(|agent| (agent.id, sync_only_verdict(snap, agent))),
    );
    out
}

fn build_entry<'a>(
    snap: &'a AppStateSnapshot,
    host: &'static dyn crate::integration::HostApp,
) -> HostEntryPayload<'a> {
    let st = snap.hosts.get(host.id());
    let (clients, view) = model_view(snap, host);
    let snapshot = st.and_then(|s| s.snapshot.as_ref());
    let verdict = host_verdict(snap, host, &view);
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
        enabled: is_enabled(snap, host.id()),
        last_generated_profile: st.and_then(|s| s.last_generated_profile.as_ref()),
        health: snapshot.map(HostHealthPayload::from),
        compatible_models: view.compatible_models,
        models_checked: view.checked,
        compatible_models_available: view.available,
        unconfigured_providers: view.unconfigured_providers,
        model_protocols: clients.tags(),
        model_protocols_overridden: clients.filtered,
        cowork_session: cowork_session(host.id()),
        surface: AgentSurface::LocalProfile,
        verdict,
    }
}

fn build_sync_only_entry<'a>(
    snap: &'a AppStateSnapshot,
    agent: &'a SyncOnlyAgent,
) -> HostEntryPayload<'a> {
    let verdict = sync_only_verdict(snap, agent);
    let caps = HostCapabilities::for_surface(AgentSurface::SyncOnly, false);
    HostEntryPayload {
        id: agent.id,
        display_name: agent.display_name,
        kind: agent.id.kind(),
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
        enabled: is_enabled(snap, agent.id),
        last_generated_profile: None,
        health: None,
        compatible_models: Vec::new(),
        models_checked: false,
        compatible_models_available: false,
        unconfigured_providers: Vec::new(),
        model_protocols: Vec::new(),
        model_protocols_overridden: false,
        cowork_session: None,
        surface: AgentSurface::SyncOnly,
        verdict,
    }
}

pub(crate) fn single_host_payload(
    snap: &AppStateSnapshot,
    host_id: HostKind,
) -> Option<HostEntryPayload<'_>> {
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
    entries.retain(|e| e.enabled);
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
