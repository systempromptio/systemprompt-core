//! Host-app status as the webview receives it: the local proxy, each agent's
//! verdict, and the folded fleet summary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::bridge::host::HostKind;

use crate::integration::agent_fleet::AgentFleets;
use crate::integration::agent_health::{AgentSurface, AgentVerdict};
use crate::integration::host_app::{AppInstallState, ConfigFormat, HostAppKind, ProfileCode};
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct HostHealthPayload<'a> {
    pub profile: Verdict<ProfileCode>,
    pub missing_required: &'a [String],
    pub app: Verdict<AppInstallState>,
    pub host_running: Option<bool>,
    pub probe_error: Option<&'a str>,
    pub host_processes: &'a [String],
    pub inference_models: Vec<String>,
    pub inference_models_error: Option<String>,
    pub probed_at_unix: u64,
}

fn written_inference_models(raw: Option<&String>) -> (Vec<String>, Option<String>) {
    let Some(raw) = raw.map(|r| r.as_str().trim()).filter(|r| !r.is_empty()) else {
        return (Vec::new(), None);
    };
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(models) => (models, None),
        Err(e) => (
            Vec::new(),
            Some(format!("inferenceModels is not a JSON array: {e}")),
        ),
    }
}

impl<'a> From<&'a HostAppSnapshot> for HostHealthPayload<'a> {
    fn from(s: &'a HostAppSnapshot) -> Self {
        let (inference_models, inference_models_error) =
            written_inference_models(s.profile_keys.get("inferenceModels"));
        Self {
            profile: s.profile_state.verdict(),
            missing_required: s.profile_state.missing_required(),
            app: s.app_installed.verdict(),
            host_running: s.host_running,
            probe_error: s.probe_error.as_deref(),
            host_processes: &s.host_processes,
            inference_models,
            inference_models_error,
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
    pub hosts_gated: bool,
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
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub id: HostKind,
    pub display_name: &'a str,
    #[cfg_attr(feature = "ts-export", ts(type = "\"desktop-app\" | \"cli-tool\""))]
    pub kind: HostAppKind,
    pub description: &'a str,
    pub icon: &'a str,
    pub config_format: ConfigFormat,
    pub download_url: &'a str,
    pub install_action_label: &'a str,
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
    pub cowork_session: Option<CoworkSessionPayload>,
    pub surface: AgentSurface,
    pub verdict: AgentVerdict,
}

/// The Cowork session Claude Desktop's org plugins are enabled in.
///
/// `source` names the rule that chose it: `pinned` (`cowork.session_org_dir`),
/// `gateway` (the profile's `organization_uuid`), `personal` or
/// `only-session`.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct CoworkSessionPayload {
    pub org: String,
    pub path: String,
    pub source: &'static str,
}
