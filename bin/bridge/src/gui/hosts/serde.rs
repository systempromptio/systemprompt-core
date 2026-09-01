//! JSON payload shapes for host-app status sent to the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::gui::state::AppStateSnapshot;
use crate::integration::agent_health::{
    AgentFleets, AgentSurface, AgentVerdict, HostHealthInputs, HostModelViewRef, SYNC_ONLY_AGENTS,
};
use crate::integration::host_app::{AppInstallState, ConfigFormat, HostKind, ProfileCode};
use crate::integration::proxy_probe::ProxyProbeState;
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};
use crate::verdict::Verdict;

#[derive(Serialize)]
pub(crate) struct ProxyPayload<'a> {
    #[serde(flatten)]
    health: &'a ProxyHealth,
    verdict: Verdict<ProxyProbeState>,
    governing: bool,
}

impl<'a> From<&'a ProxyHealth> for ProxyPayload<'a> {
    fn from(health: &'a ProxyHealth) -> Self {
        Self {
            health,
            verdict: health.state.verdict(),
            governing: health.state.governing(),
        }
    }
}

// Why: the probe's raw snapshot no longer crosses the wire. The drawer used to
// branch on `profile_state.kind` — the same anti-pattern the verdict module
// forbids — so what it needs is shipped as verdicts and plain facts instead.
#[derive(Serialize)]
pub(crate) struct HostHealthPayload<'a> {
    profile: Verdict<ProfileCode>,
    missing_required: &'a [String],
    app: Verdict<AppInstallState>,
    host_running: bool,
    host_processes: &'a [String],
    inference_models: Vec<String>,
    probed_at_unix: u64,
}

impl<'a> From<&'a HostAppSnapshot> for HostHealthPayload<'a> {
    fn from(s: &'a HostAppSnapshot) -> Self {
        Self {
            profile: s.profile_state.verdict(),
            missing_required: s.profile_state.missing_required(),
            app: s.app_installed.verdict(),
            host_running: s.host_running,
            host_processes: &s.host_processes,
            inference_models: s
                .profile_keys
                .get("inferenceModels")
                .map(|raw| {
                    raw.split(',')
                        .map(str::trim)
                        .filter(|m| !m.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            probed_at_unix: s.probed_at_unix,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct HostsPayload<'a> {
    pub host_apps: Vec<HostEntryPayload<'a>>,
    pub local_proxy: ProxyPayload<'a>,
    // Why: the gate comes from the last signed manifest, which does not exist
    // before the first sync, so on a fresh install `host_apps` is every host
    // this build registers rather than the subset this installation permits.
    // Surfaces that offer to *act* on a host must fail closed while this is
    // false. It reads `manifest_synced`, never `enabled_hosts` being non-empty:
    // an instance may legitimately disable every host, and that empty list is a
    // real answer, not a missing one.
    pub hosts_gated: bool,
    // Why: folded from the very verdicts in `host_apps`, so the summary card and the
    // rows cannot disagree.
    pub agent_fleet: AgentFleets,
    pub agents_onboarded: bool,
    pub first_run: crate::gui::first_run::serde::FirstRunPayload<'a>,
}

#[derive(Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire payload of independent per-host facts the GUI renders verbatim"
)]
pub(crate) struct HostEntryPayload<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
    pub kind: HostKind,
    pub description: &'a str,
    pub icon: &'a str,
    pub config_format: ConfigFormat,
    pub download_url: &'a str,
    pub install_action_label: &'a str,
    pub can_open: bool,
    pub probe_in_flight: bool,
    pub enabled: bool,
    pub last_generated_profile: Option<&'a GeneratedProfile>,
    pub health: Option<HostHealthPayload<'a>>,
    pub compatible_models: Vec<String>,
    pub models_checked: bool,
    pub compatible_models_available: bool,
    pub unconfigured_providers: Vec<String>,
    pub model_protocols: Vec<String>,
    pub model_protocols_overridden: bool,
    pub surface: AgentSurface,
    pub verdict: AgentVerdict,
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
    HostEntryPayload {
        id: host.id(),
        display_name: host.display_name(),
        kind: host.kind(),
        description: host.description(),
        icon: host.icon_id(),
        config_format: host.config_format(),
        download_url: host.download_url(),
        install_action_label: host.install_action_label(),
        can_open: host.can_open(),
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
    HostEntryPayload {
        id: agent.id,
        display_name: agent.display_name,
        kind: HostKind::CliTool,
        description: agent.description,
        icon: agent.icon,
        config_format: ConfigFormat::Json,
        download_url: "",
        install_action_label: "",
        can_open: false,
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
    crate::integration::host_apps()
        .iter()
        .copied()
        .find(|h| h.id() == host_id)
        .map(|host| build_entry(snap, host))
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
