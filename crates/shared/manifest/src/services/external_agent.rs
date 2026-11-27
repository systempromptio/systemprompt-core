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
//! A host that configures itself (`harness`) is served only when its entry is
//! present with `enabled: true`; every other host is served unless its entry
//! says `enabled: false`.
//!
//! `extensions` names the optional feature extensions a host compiled with
//! them may turn on; the signed manifest carries the list as
//! `host_extensions` for each host the user has enabled. An extension adds
//! capability and nothing else: a host that is not told of one runs without
//! it, so nothing that restricts or enforces may be an extension. Only an
//! entry whose key names a host that configures itself may declare any, and
//! each id is a kebab-case [`HostExtensionId`] refused at load when malformed.
//!
//! [`validate_catalog`] refuses an entry whose key differs from its `id`, and
//! two entries whose keys name one host (`codex_cli` beside `codex-cli`):
//! which of the two would supply `enabled` and the signed extension list is
//! otherwise an accident of map order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};
use systemprompt_models::bridge::ids::HostExtensionId;
use systemprompt_models::errors::{ServicesValidationError, ValidationSubject};

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
    pub extensions: Vec<HostExtensionId>,
}

#[must_use]
pub fn external_agent_host(key: &ExternalAgentId) -> Option<HostKind> {
    key.as_str().replace('_', "-").parse().ok()
}

impl ExternalAgentConfig {
    pub fn validate(&self, key: &ExternalAgentId) -> Result<(), ServicesValidationError> {
        let subject = || ValidationSubject::named("External agent", key.as_str());
        if self.id.as_str() != key.as_str() {
            return Err(ServicesValidationError::invalid_field(
                subject(),
                format!(
                    "id '{}' differs from its key '{}'",
                    self.id.as_str(),
                    key.as_str()
                ),
            ));
        }
        if self.extensions.is_empty() {
            return Ok(());
        }
        if external_agent_host(key).is_none_or(|host| host.config_owner() != ConfigOwner::Host) {
            return Err(ServicesValidationError::invalid_field(
                subject(),
                format!(
                    "extensions are declared but '{}' names no host that configures itself",
                    key.as_str()
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for extension in &self.extensions {
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

pub fn validate_catalog(
    agents: &HashMap<ExternalAgentId, ExternalAgentConfig>,
) -> Result<(), ServicesValidationError> {
    let mut keys: Vec<&ExternalAgentId> = agents.keys().collect();
    keys.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut by_host: BTreeMap<HostKind, &ExternalAgentId> = BTreeMap::new();
    for key in keys {
        if let Some(agent) = agents.get(key) {
            agent.validate(key)?;
        }
        let Some(host) = external_agent_host(key) else {
            continue;
        };
        if let Some(other) = by_host.insert(host, key) {
            return Err(ServicesValidationError::invalid_field(
                ValidationSubject::named("External agent", key.as_str()),
                format!(
                    "'{}' and '{}' both name the host '{host}'; keep one entry",
                    other.as_str(),
                    key.as_str()
                ),
            ));
        }
    }
    Ok(())
}

const fn default_enabled() -> bool {
    true
}
