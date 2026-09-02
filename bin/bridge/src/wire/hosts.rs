//! Host-app status as the webview receives it: the local proxy, each agent's
//! verdict, and the folded fleet summary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::integration::agent_fleet::AgentFleets;
use crate::integration::agent_health::{AgentSurface, AgentVerdict};
use crate::integration::host_app::{AppInstallState, ConfigFormat, HostKind, ProfileCode};
use crate::integration::{GeneratedProfile, HostAppSnapshot, ProxyHealth};
use crate::proxy_probe::ProxyProbeState;
use crate::verdict::Verdict;
use crate::wire::first_run::FirstRunPayload;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct ProxyPayload<'a> {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub health: &'a ProxyHealth,
    pub verdict: Verdict<ProxyProbeState>,
    pub governing: bool,
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
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostHealthPayload<'a> {
    pub profile: Verdict<ProfileCode>,
    pub missing_required: &'a [String],
    pub app: Verdict<AppInstallState>,
    pub host_running: bool,
    pub host_processes: &'a [String],
    pub inference_models: Vec<String>,
    pub probed_at_unix: u64,
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostsPayload<'a> {
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
    pub first_run: FirstRunPayload<'a>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire payload of independent per-host facts the GUI renders verbatim"
)]
pub struct HostEntryPayload<'a> {
    pub id: &'a str,
    pub display_name: &'a str,
    pub kind: HostKind,
    pub description: &'a str,
    pub icon: &'a str,
    pub config_format: ConfigFormat,
    pub download_url: &'a str,
    pub install_action_label: &'a str,
    // Why: what the GUI may offer for this host, decided here rather than in
    // the front end. A sync-only agent has no local profile, no config file and
    // nothing installed, so every one of these is false for it — and the drawer
    // used to render Open / Repair / Verify / Show config / Remove regardless,
    // each of which reached a handler that could only answer
    // "unknown host: claude-code". Deriving the affordance a second time in JS
    // (`surface === "sync-only"`) is how the two answers drift apart.
    pub can_open: bool,
    pub can_verify: bool,
    pub can_repair: bool,
    pub can_open_config: bool,
    pub can_remove: bool,
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
