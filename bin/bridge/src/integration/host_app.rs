use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileState {
    Absent,
    Partial { missing_required: Vec<String> },
    Installed,
}

impl ProfileState {
    #[must_use]
    pub const fn is_installed(&self) -> bool {
        matches!(self, Self::Installed)
    }

    #[must_use]
    pub fn from_keys(required: &[&str], present: &BTreeMap<String, String>) -> Self {
        if present.is_empty() {
            return Self::Absent;
        }
        let missing: Vec<String> = required
            .iter()
            .filter(|k| !present.contains_key(**k))
            .map(|k| (*k).to_string())
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

#[derive(Debug, Clone, Serialize)]
pub struct HostAppSnapshot {
    pub host_id: &'static str,
    pub display_name: &'static str,
    pub profile_state: ProfileState,
    pub profile_source: Option<String>,
    pub profile_keys: BTreeMap<String, String>,
    pub host_running: bool,
    pub host_processes: Vec<String>,
    pub app_installed: bool,
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

    /// Official download page, opened externally when the desktop app is not
    /// installed. Empty means no download action is offered.
    fn download_url(&self) -> &'static str {
        ""
    }

    /// Wire-protocol tags whose provider models this host can use, e.g.
    /// `["anthropic"]` for Claude Desktop or `["openai-chat",
    /// "openai-responses"]` for Codex. An empty slice means no restriction —
    /// every provider's models are offered. Drives both the generated profile's
    /// model list and the GUI's per-host "compatible models" display.
    fn accepted_protocols(&self) -> &'static [&'static str] {
        &[]
    }
}

/// A host's view of the gateway's providers, filtered to its wire protocols.
///
/// Carries the models it can use, whether any usable model comes from a
/// *configured* provider, and which matching providers still lack a credential
/// secret. `checked` guards the others: it is false when there was no provider
/// health to evaluate (e.g. the gateway was unreachable), so the UI can tell
/// "nothing usable" apart from "not yet checked" rather than crying wolf on
/// startup.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostModelView {
    pub compatible_models: Vec<String>,
    pub checked: bool,
    pub available: bool,
    pub unconfigured_providers: Vec<String>,
}

/// Project per-provider `health` onto one host, keeping only providers whose
/// wire protocol the host speaks (`accepted`; empty means no restriction).
/// Model order is preserved and duplicates dropped.
///
/// Reached only from the GUI host views (macOS/Windows), so it is gated to
/// those targets plus `test` to keep it off the unused-code list on Linux.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[must_use]
pub fn host_model_view(
    health: &[crate::auth::types::ProviderHealth],
    accepted: &[&str],
) -> HostModelView {
    let mut seen = std::collections::HashSet::new();
    let mut view = HostModelView {
        checked: !health.is_empty(),
        ..HostModelView::default()
    };
    for provider in health {
        let speaks = accepted.is_empty() || accepted.contains(&provider.protocol.as_str());
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

#[cfg(test)]
mod tests {
    use super::{HostModelView, host_model_view};
    use crate::auth::types::ProviderHealth;

    fn ph(name: &str, protocol: &str, configured: bool, models: &[&str]) -> ProviderHealth {
        ProviderHealth {
            name: name.to_owned(),
            protocol: protocol.to_owned(),
            configured,
            models: models.iter().map(|s| (*s).to_owned()).collect(),
            config_issue: (!configured).then(|| "missing".to_owned()),
        }
    }

    #[test]
    fn filters_to_accepted_protocol() {
        let health = vec![
            ph("anthropic", "anthropic", true, &["claude-sonnet-4-6"]),
            ph("gemini", "gemini", true, &["gemini-3.1-flash-lite-preview"]),
            ph("openai", "openai-responses", true, &["gpt-5"]),
        ];

        assert_eq!(
            host_model_view(&health, &["anthropic"]).compatible_models,
            vec!["claude-sonnet-4-6".to_owned()]
        );
        assert_eq!(
            host_model_view(&health, &["openai-chat", "openai-responses"]).compatible_models,
            vec!["gpt-5".to_owned()]
        );
    }

    #[test]
    fn flags_unconfigured_matching_provider() {
        let health = vec![ph("anthropic", "anthropic", false, &["claude-sonnet-4-6"])];

        let view = host_model_view(&health, &["anthropic"]);
        assert!(view.checked);
        assert!(!view.available);
        assert_eq!(view.unconfigured_providers, vec!["anthropic".to_owned()]);
    }

    #[test]
    fn available_only_counts_matching_protocol() {
        let health = vec![
            ph("openai", "openai-responses", true, &["gpt-5"]),
            ph("anthropic", "anthropic", false, &["claude-sonnet-4-6"]),
        ];

        let view = host_model_view(&health, &["anthropic"]);
        assert!(!view.available);
        assert_eq!(view.compatible_models, vec!["claude-sonnet-4-6".to_owned()]);
    }

    #[test]
    fn unchecked_when_no_health() {
        assert_eq!(
            host_model_view(&[], &["anthropic"]),
            HostModelView::default()
        );
    }
}
