//! Pure JSON construction for
//! `marketplaces/<name>/.claude-plugin/marketplace.json`.
//!
//! Standard Claude-Code marketplace format. The bridge owns the whole file
//! (we are the marketplace author), so unlike the registry this is a clean
//! render — no foreign-field preservation needed.

use serde::{Deserialize, Serialize};

use super::CoworkPluginsError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceFile {
    pub name: String,
    pub owner: MarketplaceOwner,
    pub plugins: Vec<MarketplacePluginEntry>,
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
    pub source: super::registry::LocalSource,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub fn render_marketplace(file: &MarketplaceFile) -> Result<Vec<u8>, CoworkPluginsError> {
    serde_json::to_vec_pretty(file).map_err(CoworkPluginsError::JsonParse)
}
