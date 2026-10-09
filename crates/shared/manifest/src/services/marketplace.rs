//! Marketplace manifest configuration and access assignment block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use systemprompt_identifiers::MarketplaceId;

pub use super::marketplace_access::MarketplaceAccess;
pub use super::marketplace_claude_code::ClaudeCodeMarketplaceConfig;
pub use super::marketplace_external::{ExternalMarketplace, ExternalMarketplaceSource};
use super::marketplace_external_plugin::{ExternalPluginEntry, validate_external_plugins};
use super::plugin::PluginAuthor;
use systemprompt_models::errors::ServicesValidationError;
use systemprompt_models::plugin::PluginComponentRef;

const fn default_true() -> bool {
    true
}

/// The membership lists a [`MarketplaceConfig`] declares directly.
///
/// Skills are deliberately absent: a marketplace selects plugins, and skill
/// membership is derived from what those plugins ship
/// (`ServicesConfig::marketplace_skill_members`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceMemberKind {
    Plugins,
    Agents,
    McpServers,
    Artifacts,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceVisibility {
    #[default]
    Public,
    Private,
    Org,
}

/// Whether a [`MarketplaceAccessRule`] grants or refuses the dimension values
/// it names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceRuleAccess {
    #[default]
    Allow,
    Deny,
}

impl MarketplaceRuleAccess {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

/// One attribute-dimension grant on a marketplace: `rule_type` is an extension
/// subject-dimension slug, and each entry in `values` becomes one
/// `access_control_rules` row for the marketplace entity.
///
/// Roles keep their own `access.roles` list, so `rule_type` may name neither
/// `role` nor `user`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceAccessRule {
    pub rule_type: String,
    pub values: Vec<String>,
    #[serde(default)]
    pub access: MarketplaceRuleAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceConfigFile {
    pub marketplace: MarketplaceConfig,
}

/// Marketplace manifest configuration.
///
/// Skills are deliberately absent: a marketplace selects plugins, and skills
/// follow the plugins that ship them. A marketplace-level skill list would be
/// a second source of truth that can silently diverge from what plugin
/// bundles actually deliver, so `deny_unknown_fields` rejects a stale
/// `skills:` block outright.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceConfig {
    pub id: MarketplaceId,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub author: PluginAuthor,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub license: String,
    #[serde(default)]
    pub visibility: MarketplaceVisibility,

    #[serde(default)]
    pub plugins: PluginComponentRef,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub agents: PluginComponentRef,
    #[serde(default)]
    pub artifacts: PluginComponentRef,

    #[serde(default)]
    pub access: MarketplaceAccess,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_cross_marketplace_dependencies_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_marketplaces: Vec<ExternalMarketplace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_plugins: Vec<ExternalPluginEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<ClaudeCodeMarketplaceConfig>,
}

impl MarketplaceConfig {
    #[must_use]
    pub const fn members(&self, kind: MarketplaceMemberKind) -> &PluginComponentRef {
        match kind {
            MarketplaceMemberKind::Plugins => &self.plugins,
            MarketplaceMemberKind::Agents => &self.agents,
            MarketplaceMemberKind::McpServers => &self.mcp_servers,
            MarketplaceMemberKind::Artifacts => &self.artifacts,
        }
    }

    pub fn validate(&self, key: &str) -> Result<(), ServicesValidationError> {
        let id_str = self.id.as_str();
        if id_str.len() < 3 || id_str.len() > 50 {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': id must be between 3 and 50 characters"
            )));
        }

        if !id_str
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': id must be lowercase alphanumeric with hyphens only \
                 (kebab-case)"
            )));
        }

        if self.version.is_empty() {
            return Err(ServicesValidationError::required(format!(
                "Marketplace '{key}': version must not be empty"
            )));
        }

        self.access.validate(key)?;
        self.validate_external_marketplaces(key)?;
        validate_external_plugins(&self.external_plugins, &self.plugins.include, key)?;
        if let Some(claude_code) = &self.claude_code {
            claude_code.validate(key)?;
        }

        Ok(())
    }

    fn validate_external_marketplaces(&self, key: &str) -> Result<(), ServicesValidationError> {
        let mut names = BTreeSet::new();
        for external in &self.external_marketplaces {
            external.validate(key)?;
            let name = external.name.trim();
            if name == self.id.as_str() {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': external_marketplaces may not reuse this \
                     marketplace's own id"
                )));
            }
            if !names.insert(name) {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': external marketplace '{name}' is declared twice"
                )));
            }
        }
        for allowed in &self.allow_cross_marketplace_dependencies_on {
            if allowed.trim().is_empty() {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': allow_cross_marketplace_dependencies_on must not \
                     contain blank entries"
                )));
            }
            if allowed == self.id.as_str() {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': allow_cross_marketplace_dependencies_on names this \
                     marketplace itself"
                )));
            }
        }
        Ok(())
    }
}
