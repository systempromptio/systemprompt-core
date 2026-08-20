//! `HostApp` trait: per-host credential and profile integration contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;
use systemprompt_models::profile::ApiSurface;

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
    pub headers: BTreeMap<String, String>,
}

/// Why a complete profile still cannot work.
///
/// Both reasons produce an identical symptom — every request 403s with "bad
/// loopback secret" — and an identical fix, so they share a state. They are
/// distinguished only so the message can name the cause.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaleReason {
    LoopbackSecret,
    ProxyPort,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileState {
    Absent,
    Partial { missing_required: Vec<String> },
    Installed,
    Stale { reason: StaleReason },
}

impl ProfileState {
    #[must_use]
    pub const fn is_installed(&self) -> bool {
        matches!(self, Self::Installed)
    }

    #[must_use]
    pub fn classify(
        required: &[&str],
        present: &BTreeMap<String, String>,
        secret_fresh: Option<bool>,
        endpoint_fresh: Option<bool>,
    ) -> Self {
        match Self::from_keys(required, present) {
            Self::Installed if secret_fresh == Some(false) => Self::Stale {
                reason: StaleReason::LoopbackSecret,
            },
            Self::Installed if endpoint_fresh == Some(false) => Self::Stale {
                reason: StaleReason::ProxyPort,
            },
            state => state,
        }
    }

    #[must_use]
    pub fn endpoint_freshness(configured_url: Option<&str>) -> Option<bool> {
        use crate::integration::proxy_probe::{PortMatch, classify_configured_port};
        let url = configured_url.filter(|u| !u.is_empty())?;
        match classify_configured_port(url, crate::proxy::resolved_port()) {
            PortMatch::Match => Some(true),
            PortMatch::Mismatch { .. } => Some(false),
            PortMatch::NotLoopback | PortMatch::Unparseable => None,
        }
    }

    fn from_keys(required: &[&str], present: &BTreeMap<String, String>) -> Self {
        if present.is_empty() {
            return Self::Absent;
        }
        let missing: Vec<String> = required
            .iter()
            .filter(|k| !present.contains_key(**k))
            .map(|k| (*k).to_owned())
            .collect();
        if missing.is_empty() {
            Self::Installed
        } else {
            Self::Partial {
                missing_required: missing,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HostConfigSchema {
    pub required_keys: &'static [&'static str],
    pub display_keys: &'static [&'static str],
}

/// Outcome of looking for the host's application on disk.
///
/// [`Self::Unknown`] is not a synonym for "absent": it means every detector we
/// tried was inconclusive (a bounded probe timed out, a registry hive was
/// unreadable). Callers must never render it as "not installed" — an
/// inconclusive probe masking an otherwise healthy host is the bug this
/// tri-state exists to prevent.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppInstallState {
    Installed,
    NotInstalled,
    Unknown,
}

impl AppInstallState {
    #[must_use]
    pub const fn is_installed(self) -> bool {
        matches!(self, Self::Installed)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAppSnapshot {
    pub host_id: &'static str,
    pub display_name: &'static str,
    pub profile_state: ProfileState,
    pub profile_source: Option<String>,
    pub profile_keys: BTreeMap<String, String>,
    pub host_running: bool,
    pub host_processes: Vec<String>,
    pub app_installed: AppInstallState,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedProfile {
    pub path: String,
    pub bytes: usize,
    pub payload_uuid: String,
    pub profile_uuid: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    DesktopApp,
    CliTool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFormat {
    Json,
    Toml,
    Plist,
    Reg,
}

pub trait HostApp: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn config_schema(&self) -> &'static HostConfigSchema;
    fn probe(&self) -> HostAppSnapshot;
    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile>;
    fn install_profile(&self, path: &str) -> std::io::Result<()>;
    fn install_action_label(&self) -> &'static str;

    fn open(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "open not implemented",
        ))
    }

    fn kind(&self) -> HostKind {
        HostKind::DesktopApp
    }

    fn description(&self) -> &'static str {
        ""
    }

    fn icon_id(&self) -> &'static str {
        self.id()
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        ""
    }

    fn accepted_surfaces(&self) -> &'static [ApiSurface] {
        &[]
    }
}

/// `checked` is false when there was no provider health to evaluate
/// (distinguishes "nothing usable" from "not yet checked").
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostModelView {
    pub compatible_models: Vec<String>,
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: Vec<String>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[must_use]
pub fn host_model_view(
    health: &[crate::auth::types::ProviderHealth],
    accepted: &[ApiSurface],
) -> HostModelView {
    let mut seen = std::collections::HashSet::new();
    let mut view = HostModelView {
        checked: !health.is_empty(),
        ..HostModelView::default()
    };
    for provider in health {
        let speaks = accepted.is_empty() || accepted.contains(&provider.surface);
        if !speaks {
            continue;
        }
        if !provider.configured {
            view.unconfigured_providers.push(provider.name.clone());
        } else if !provider.models.is_empty() {
            view.available = true;
        }
        for model in &provider.models {
            if seen.insert(model.clone()) {
                view.compatible_models.push(model.clone());
            }
        }
    }
    view
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[must_use]
pub fn effective_surfaces(
    host_id: &str,
    default: &[ApiSurface],
    overrides: &BTreeMap<String, Vec<String>>,
) -> Vec<ApiSurface> {
    overrides.get(host_id).map_or_else(
        || default.to_vec(),
        |tags| {
            tags.iter()
                .filter_map(|t| ApiSurface::from_tag(t))
                .collect()
        },
    )
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[must_use]
pub fn has_surface_override(host_id: &str, overrides: &BTreeMap<String, Vec<String>>) -> bool {
    overrides.contains_key(host_id)
}
