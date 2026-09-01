//! The serialized data model the Marketplace listing hands to the webview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ChangeKind {
    Installed,
    Updated,
    Removed,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum MarketplaceExtra {
    Plugin(PluginManifest),
    Frontmatter(FrontmatterExtra),
    Mcp(McpServerEntry),
    None,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceItem {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) source: &'static str,
    pub(crate) path: String,
    pub(crate) summary: Option<String>,
    pub(crate) readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) change: Option<ChangeKind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) children: Vec<PluginChild>,
    // Why: empty for plugins themselves, for MCP servers (the registry
    // snapshot is not per-plugin — `mark_shared_mcp` models that instead), and
    // for items from an external source, which render under "Ungrouped".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) plugins: Vec<String>,
    pub(crate) extra: MarketplaceExtra,
}

impl MarketplaceItem {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        summary: Option<String>,
        path: impl Into<String>,
        source: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source,
            path: path.into(),
            summary,
            readme: None,
            version: None,
            author: None,
            homepage: None,
            change: None,
            children: Vec::new(),
            plugins: Vec::new(),
            extra: MarketplaceExtra::None,
        }
    }

    #[must_use]
    pub fn with_provenance(
        mut self,
        version: Option<String>,
        author: Option<String>,
        homepage: Option<String>,
    ) -> Self {
        self.version = version;
        self.author = author;
        self.homepage = homepage;
        self
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PluginChild {
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    pub shared: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct MarketplaceDiff {
    pub(crate) installed: Vec<String>,
    pub(crate) updated: Vec<String>,
    pub(crate) removed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_applied_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceListing {
    pub(crate) plugins: Vec<MarketplaceItem>,
    pub(crate) skills: Vec<MarketplaceItem>,
    pub(crate) hooks: Vec<MarketplaceItem>,
    pub(crate) mcp: Vec<MarketplaceItem>,
    pub(crate) agents: Vec<MarketplaceItem>,
    pub(crate) artifacts: Vec<MarketplaceItem>,
    pub(crate) plugins_dir: Option<String>,
    pub(crate) last_sync_diff: MarketplaceDiff,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct PluginManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) homepage: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FrontmatterExtra {
    pub(crate) id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct McpServerEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) proxy_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) upstream_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transport: Option<String>,
}
