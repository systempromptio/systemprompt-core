//! Plugin configuration and component-reference model.
//!
//! [`PluginConfig`] is the manifest shape loaded from a plugin's config file;
//! its skill/agent/MCP/content references are [`PluginComponentRef`]s resolved
//! against the instance ([`ComponentSource`]). [`PluginSummary`] is the
//! list-view projection; [`PluginConfig::validate`] enforces id and reference
//! rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::PluginId;

use crate::errors::ConfigValidationError;

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentSource {
    Instance,
    #[default]
    Explicit,
}

impl fmt::Display for ComponentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instance => write!(f, "instance"),
            Self::Explicit => write!(f, "explicit"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentFilter {
    Enabled,
}

impl fmt::Display for ComponentFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigFile {
    pub plugin: PluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginVariableDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub id: PluginId,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub author: PluginAuthor,
    pub keywords: Vec<String>,
    pub license: String,
    pub category: String,

    pub skills: PluginComponentRef,
    pub agents: PluginComponentRef,
    #[serde(default)]
    pub rules: PluginComponentRef,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub content_sources: PluginComponentRef,
    #[serde(default)]
    pub artifacts: PluginComponentRef,
    #[serde(default)]
    pub hooks: PluginHooksRef,
    #[serde(default)]
    pub scripts: Vec<PluginScript>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PluginDependency>,
}

/// One plugin this plugin requires, in Claude Code's `plugin.json`
/// `dependencies` vocabulary.
///
/// A bare `name` resolves inside the marketplace that carries the dependant;
/// `marketplace` points at another marketplace, which the carrying
/// [`MarketplaceConfig`](super::marketplace::MarketplaceConfig) must list in
/// `allow_cross_marketplace_dependencies_on` and, when it is not one of this
/// instance's own marketplaces, declare under `external_marketplaces`.
/// `version` is a semver range Claude Code checks at install time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginDependency {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl PluginDependency {
    fn validate(&self, key: &str) -> Result<(), ConfigValidationError> {
        if self.name.trim().is_empty() {
            return Err(ConfigValidationError::required(format!(
                "Plugin '{key}': dependencies entries must name a plugin"
            )));
        }
        if self
            .marketplace
            .as_deref()
            .is_some_and(|m| m.trim().is_empty())
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Plugin '{key}': dependency '{}' sets an empty marketplace",
                self.name
            )));
        }
        if let Some(range) = &self.version
            && semver::VersionReq::parse(range).is_err()
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Plugin '{key}': dependency '{}' version '{range}' is not a semver range",
                self.name
            )));
        }
        Ok(())
    }
}

/// Selects which hooks a plugin materialises into its `hooks/hooks.json`.
///
/// Claude Code executes plugin hooks session-globally — a `PreToolUse` hook
/// with a `*` matcher fires for every tool call regardless of which plugin
/// contributed the tool. One plugin therefore carries the governance hooks for
/// the whole instance; every other plugin emits an empty hooks file.
///
/// `comms` is an opt-in on that same owner: when set, the bridge also installs
/// the `UserPromptSubmit`/`Stop` hooks that drain gateway announcements into
/// the session. Off by default — a tenant that publishes no announcements has
/// no reason to run a command on every prompt.
///
/// `evaluation` is server-side only: it switches on the session-evaluation
/// engine for the sessions this plugin's track hook reports, and installs no
/// client hook of its own. Like `comms` it rides on the governance owner, so
/// `is_empty` ignores it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginHooksRef {
    #[serde(default)]
    pub governance: bool,
    #[serde(default)]
    pub comms: bool,
    #[serde(default)]
    pub evaluation: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
}

impl PluginHooksRef {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.governance && self.include.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginComponentRef {
    #[serde(default)]
    pub source: ComponentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<ComponentFilter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginScript {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginSummary {
    pub id: PluginId,
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub skill_count: usize,
    pub agent_count: usize,
}

impl From<&PluginConfig> for PluginSummary {
    fn from(config: &PluginConfig) -> Self {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            display_name: config.name.clone(),
            enabled: config.enabled,
            skill_count: config.skills.include.len(),
            agent_count: config.agents.include.len(),
        }
    }
}

impl PluginConfig {
    pub fn validate(&self, key: &str) -> Result<(), ConfigValidationError> {
        let id_str = self.id.as_str();
        if id_str.len() < 3 || id_str.len() > 50 {
            return Err(ConfigValidationError::invalid_field(format!(
                "Plugin '{key}': id must be between 3 and 50 characters"
            )));
        }

        if !id_str
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Plugin '{key}': id must be lowercase alphanumeric with hyphens only (kebab-case)"
            )));
        }

        if self.version.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "Plugin '{key}': version must not be empty"
            )));
        }

        Self::validate_component_ref(&self.skills, key, "skills")?;
        Self::validate_component_ref(&self.agents, key, "agents")?;
        Self::validate_component_ref(&self.artifacts, key, "artifacts")?;
        Self::validate_component_ref(&self.rules, key, "rules")?;

        for dependency in &self.dependencies {
            dependency.validate(key)?;
        }
        let mut seen = std::collections::BTreeSet::new();
        for dependency in &self.dependencies {
            if !seen.insert((dependency.name.as_str(), dependency.marketplace.as_deref())) {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Plugin '{key}': dependency '{}' is listed twice",
                    dependency.name
                )));
            }
        }

        Ok(())
    }

    fn validate_component_ref(
        component: &PluginComponentRef,
        key: &str,
        field: &str,
    ) -> Result<(), ConfigValidationError> {
        if component.source == ComponentSource::Instance && !component.include.is_empty() {
            return Err(ConfigValidationError::invalid_field(format!(
                "Plugin '{key}': {field}.source is 'instance' but {field}.include is set (ignored)"
            )));
        }

        Ok(())
    }
}
