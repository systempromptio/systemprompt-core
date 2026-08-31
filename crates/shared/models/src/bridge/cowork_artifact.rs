//! Cowork's native artifact projections of a manifest [`ArtifactEntry`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::bridge::ids::{LibraryArtifactId, PluginId};
use crate::bridge::manifest::ArtifactEntry;


// Why: field names and casing must track Cowork's native `create_artifact`
// input, so a consumer can read a bundle's `artifacts/<id>.json` and the
// bridge's staged library records with one parser.
#[derive(Debug, Serialize)]
pub struct CoworkLibraryArtifactRecord<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub version: &'a str,
    pub content: &'a str,
    #[serde(rename = "isStarred")]
    pub is_starred: bool,
    #[serde(rename = "mcpTools")]
    pub mcp_tools: &'a [String],
    // Why: not part of Cowork's `create_artifact` input — an additive field Cowork
    // ignores, carried so the bridge's Marketplace listing can group a stored
    // record by its owning plugin without re-fetching the manifest.
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    pub plugins: &'a [PluginId],
}

impl<'a> From<&'a ArtifactEntry> for CoworkLibraryArtifactRecord<'a> {
    fn from(a: &'a ArtifactEntry) -> Self {
        Self {
            id: a.id.as_str(),
            name: &a.name,
            description: &a.description,
            version: &a.version,
            content: &a.content,
            is_starred: a.starred,
            mcp_tools: &a.mcp_tools,
            plugins: &a.plugins,
        }
    }
}

// Why: the install manifest a plugin bundle ships at `artifacts/manifest.json`
// — every record minus its HTML, which sits beside it as `artifacts/<id>.html`
// so a seed skill can copy a page without parsing JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoworkArtifactBundleManifest {
    pub artifacts: Vec<CoworkArtifactBundleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoworkArtifactBundleRecord {
    pub id: LibraryArtifactId,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(rename = "isStarred")]
    pub is_starred: bool,
    #[serde(rename = "mcpTools")]
    pub mcp_tools: Vec<String>,
}

impl From<&ArtifactEntry> for CoworkArtifactBundleRecord {
    fn from(a: &ArtifactEntry) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            description: a.description.clone(),
            version: a.version.clone(),
            is_starred: a.starred,
            mcp_tools: a.mcp_tools.clone(),
        }
    }
}
