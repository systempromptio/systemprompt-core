use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::MarketplaceId;

use super::plugin::{PluginAuthor, PluginComponentRef};
use crate::errors::ConfigValidationError;

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceVisibility {
    #[default]
    Public,
    Private,
    Org,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceConfigFile {
    pub marketplace: MarketplaceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub skills: PluginComponentRef,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub agents: PluginComponentRef,
}

impl MarketplaceConfig {
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

        Ok(())
    }
}
