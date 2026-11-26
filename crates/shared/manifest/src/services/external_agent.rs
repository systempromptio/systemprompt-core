//! External-agent ("super-agent") catalog config.
//!
//! External agents are native apps and CLI tools that run **off** our
//! infrastructure (Claude Desktop, Codex CLI, Claude Code) and connect to the
//! gateway via the systemprompt-bridge binary
//! (`systemprompt-core/bin/bridge`). They are intentionally distinct from
//! [`AgentConfig`](super::AgentConfig), which describes server-side A2A agents
//! we run.
//!
//! The bridge owns its own static `HostApp` registry; this gateway-side YAML
//! is the catalog the admin UI lists, the operator's `enabled` flag, and
//! `default_enabled: false` for a host each user must turn on.
//! The `id` field MUST equal the bridge's `HostApp::id()` for the same host
//! (e.g. `claude_desktop`, `codex_cli`).
//!
//! `extensions` names the optional feature extensions a host compiled with
//! them may turn on; the signed manifest carries the list as
//! `host_extensions` for each host the user has enabled. Only an entry that
//! names a known host may declare extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::errors::{ServicesValidationError, ValidationSubject};

const EXTENSION_ID_MAX_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentKind {
    DesktopApp,
    CliTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAgentConfig {
    pub id: ExternalAgentId,
    pub display_name: String,
    pub kind: ExternalAgentKind,
    pub enabled: bool,
    #[serde(default = "default_enabled")]
    pub default_enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub docs_url: Option<String>,
    #[serde(default)]
    pub desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}

#[must_use]
pub fn external_agent_host(key: &ExternalAgentId) -> Option<HostKind> {
    key.as_str().replace('_', "-").parse().ok()
}

impl ExternalAgentConfig {
    pub fn validate(&self, key: &ExternalAgentId) -> Result<(), ServicesValidationError> {
        if self.extensions.is_empty() {
            return Ok(());
        }
        let subject = || ValidationSubject::named("External agent", key.as_str());
        if external_agent_host(key).is_none() {
            return Err(ServicesValidationError::invalid_field(
                subject(),
                format!(
                    "extensions are declared but '{}' names no known host",
                    key.as_str()
                ),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for extension in &self.extensions {
            if !is_extension_id(extension) {
                return Err(ServicesValidationError::invalid_field(
                    subject(),
                    format!(
                        "extension '{extension}' must be 1 to {EXTENSION_ID_MAX_CHARS} lowercase \
                         letters, digits or hyphens, starting with a letter"
                    ),
                ));
            }
            if !seen.insert(extension.as_str()) {
                return Err(ServicesValidationError::invalid_field(
                    subject(),
                    format!("extension '{extension}' is listed twice"),
                ));
            }
        }
        Ok(())
    }
}

fn is_extension_id(value: &str) -> bool {
    value.len() <= EXTENSION_ID_MAX_CHARS
        && value.starts_with(|c: char| c.is_ascii_lowercase())
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

const fn default_enabled() -> bool {
    true
}
