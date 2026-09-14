//! The serialized data model the Marketplace listing hands to the webview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Installed,
    Updated,
    Removed,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum MarketplaceExtra {
    Plugin(PluginManifest),
    Frontmatter(FrontmatterExtra),
    Mcp(McpServerEntry),
    None,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource {
    Tenant,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChildKind {
    Skills,
    Agents,
    Hooks,
    Mcp,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceItem {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) source: ItemSource,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) plugins: Vec<String>,
    pub(crate) extra: MarketplaceExtra,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl MarketplaceItem {
    #[must_use]
    pub fn builder(id: impl Into<String>, path: impl Into<String>) -> MarketplaceItemBuilder {
        let id = id.into();
        MarketplaceItemBuilder {
            item: Self {
                name: id.clone(),
                id,
                source: ItemSource::Tenant,
                path: path.into(),
                summary: None,
                readme: None,
                version: None,
                author: None,
                homepage: None,
                change: None,
                children: Vec::new(),
                plugins: Vec::new(),
                extra: MarketplaceExtra::None,
                error: None,
            },
        }
    }

    #[must_use]
    pub(crate) fn failed(id: &str, path: &std::path::Path, error: &std::io::Error) -> Self {
        Self::builder(id, path.display().to_string())
            .error(error.to_string())
            .build()
    }
}

#[derive(Debug)]
pub struct MarketplaceItemBuilder {
    item: MarketplaceItem,
}

impl MarketplaceItemBuilder {
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.item.name = name.into();
        self
    }

    #[must_use]
    pub fn summary(mut self, summary: Option<String>) -> Self {
        self.item.summary = summary;
        self
    }

    #[must_use]
    pub fn readme(mut self, readme: Option<String>) -> Self {
        self.item.readme = readme;
        self
    }

    #[must_use]
    pub fn provenance(
        mut self,
        version: Option<String>,
        author: Option<String>,
        homepage: Option<String>,
    ) -> Self {
        self.item.version = version;
        self.item.author = author;
        self.item.homepage = homepage;
        self
    }

    #[must_use]
    pub const fn change(mut self, change: ChangeKind) -> Self {
        self.item.change = Some(change);
        self
    }

    #[must_use]
    pub fn children(mut self, children: Vec<PluginChild>) -> Self {
        self.item.children = children;
        self
    }

    #[must_use]
    pub fn plugins(mut self, plugins: Vec<String>) -> Self {
        self.item.plugins = plugins;
        self
    }

    #[must_use]
    pub fn extra(mut self, extra: MarketplaceExtra) -> Self {
        self.item.extra = extra;
        self
    }

    #[must_use]
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.item.error = Some(error.into());
        self
    }

    #[must_use]
    pub fn build(self) -> MarketplaceItem {
        self.item
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PluginChild {
    pub kind: ChildKind,
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
    pub(crate) rules: Vec<MarketplaceItem>,
    pub(crate) plugins_dir: Option<String>,
    pub(crate) last_sync_diff: MarketplaceDiff,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_sync_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct PluginManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(
        default,
        deserialize_with = "author_display",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) homepage: Option<String>,
}

// Why: `author` in a Claude `plugin.json` is either a bare string or an object
// with `name`/`email`, and one object-form bundle failed the whole listing.
fn author_display<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Author {
        Name(String),
        Object {
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            email: Option<String>,
        },
    }

    Ok(match Option::<Author>::deserialize(deserializer)? {
        None => None,
        Some(Author::Name(name)) => Some(name),
        Some(Author::Object { name, email }) => name.or(email),
    })
}

#[derive(Debug, Serialize)]
pub struct FrontmatterExtra {
    pub(crate) id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct McpServerEntry {
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
