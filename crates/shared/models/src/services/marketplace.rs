//! Marketplace manifest configuration and access assignment block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::MarketplaceId;

use super::plugin::{PluginAuthor, PluginComponentRef};
use crate::errors::ConfigValidationError;

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

/// Declarative assignment block for a marketplace.
///
/// `roles` is the only identity vector core inspects: role strings are matched
/// against `access_control_rules` for the core RBAC check, mirroring
/// [`JwtClaims::roles`](crate::auth::JwtClaims). `attributes` is an opaque,
/// dotted-namespace bag core never interprets — it is forwarded verbatim to
/// extension authz/ABAC hooks, exactly as
/// [`JwtClaims::attributes`](crate::auth::JwtClaims) is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct MarketplaceAccess {
    #[serde(default)]
    pub default_included: bool,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
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

    pub fn validate(&self, key: &str) -> Result<(), ConfigValidationError> {
        let id_str = self.id.as_str();
        if id_str.len() < 3 || id_str.len() > 50 {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': id must be between 3 and 50 characters"
            )));
        }

        if !id_str
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': id must be lowercase alphanumeric with hyphens only \
                 (kebab-case)"
            )));
        }

        if self.version.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "Marketplace '{key}': version must not be empty"
            )));
        }

        if self.access.roles.iter().any(|role| role.trim().is_empty()) {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': access.roles must not contain blank entries"
            )));
        }

        Ok(())
    }
}
