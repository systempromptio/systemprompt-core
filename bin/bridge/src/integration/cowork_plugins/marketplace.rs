//! Pure JSON construction for
//! `marketplaces/<name>/.claude-plugin/marketplace.json`.
//!
//! Shape matches the Cowork-native marketplace schema:
//! - top-level `$schema`, `name`, `description`, `metadata { description, version, pluginRoot }`, `owner`
//! - `plugins[].source` is a **plain string** path (`"./plugins/<name>"`), not a `{type,path}` object
//!
//! The bridge owns the whole file; no foreign-field preservation needed.

use serde::{Deserialize, Serialize};

use super::CoworkPluginsError;

pub const MARKETPLACE_SCHEMA_URL: &str = "https://anthropic.com/claude-code/marketplace.schema.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceFile {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MarketplaceMetadata>,
    pub owner: MarketplaceOwner,
    pub plugins: Vec<MarketplacePluginEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: String,
    #[serde(rename = "pluginRoot", skip_serializing_if = "Option::is_none")]
    pub plugin_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceOwner {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplacePluginEntry {
    pub name: String,
    pub source: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<MarketplaceOwner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

pub fn render_marketplace(file: &MarketplaceFile) -> Result<Vec<u8>, CoworkPluginsError> {
    serde_json::to_vec_pretty(file).map_err(CoworkPluginsError::JsonParse)
}
