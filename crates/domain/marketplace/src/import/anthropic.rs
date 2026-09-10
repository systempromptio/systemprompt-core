//! Inbound Anthropic authoring formats the importer reads.
//!
//! [`MarketplaceJson`] is `.claude-plugin/marketplace.json` as Claude Code
//! defines it, and [`MarketplacePluginEntry`] one of its plugin records. Only
//! `name` is required on an entry; every other key is optional and several are
//! ignored by systemprompt because the equivalent fact is derived from the
//! tree. `source` is kept as an opaque value because Anthropic permits both a
//! relative path string and a git/object form;
//! [`MarketplacePluginEntry::local_path`] is the only reading the importer
//! accepts.
//!
//! [`HooksFile`] is the `hooks/hooks.json` a plugin ships, whose body is the
//! same `HookEventsConfig` core already models.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::services::hooks::HookEventsConfig;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarketplaceOwner {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarketplaceMetadata {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "pluginRoot", alias = "plugin_root")]
    pub plugin_root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceJson {
    pub name: String,
    #[serde(default)]
    pub owner: MarketplaceOwner,
    #[serde(default)]
    pub metadata: MarketplaceMetadata,
    #[serde(default)]
    pub plugins: Vec<MarketplacePluginEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplacePluginEntry {
    pub name: String,
    #[serde(default)]
    pub source: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub author: Option<serde_json::Value>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl MarketplacePluginEntry {
    pub fn author_name(&self) -> Option<String> {
        match self.author.as_ref()? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(map) => {
                map.get("name").and_then(|v| v.as_str()).map(str::to_owned)
            },
            _ => None,
        }
    }

    pub fn author_email(&self) -> Option<String> {
        match self.author.as_ref()? {
            serde_json::Value::Object(map) => {
                map.get("email").and_then(|v| v.as_str()).map(str::to_owned)
            },
            _ => None,
        }
    }

    pub fn local_path(&self) -> Option<&str> {
        match self.source.as_ref()? {
            serde_json::Value::String(s) => Some(s.as_str()),
            serde_json::Value::Object(map) => map
                .get("path")
                .or_else(|| map.get("source"))
                .and_then(|v| v.as_str()),
            _ => None,
        }
    }

    pub fn source_is_remote(&self) -> bool {
        self.source.is_some() && self.local_path().is_none()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct HooksFile {
    #[serde(default)]
    pub(super) hooks: HookEventsConfig,
}
