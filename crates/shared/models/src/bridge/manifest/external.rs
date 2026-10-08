//! Tolerant manifest mirrors of a marketplace's external references.
//!
//! The kit-side types (`ExternalMarketplace`, `ExternalPluginEntry` in the
//! services manifest) are
//! `deny_unknown_fields` so an authoring mistake fails at import. The manifest
//! must not inherit that: a bridge refusing a key a newer gateway added would
//! refuse the whole manifest. These mirrors are flat, accept unknown keys,
//! default every optional field, and serialise back to the exact JSON object
//! Claude Code reads, so the bridge writes them verbatim.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


/// A marketplace Claude Code fetches itself, as the manifest carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExternalMarketplace {
    pub name: String,
    pub source: ManifestExternalMarketplaceSource,
}

/// The `extraKnownMarketplaces` source object, flattened so any `source`
/// kind a newer gateway sends still parses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExternalMarketplaceSource {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// The `skills` override of a pass-through entry: one path or several.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ExternalPluginSkills {
    Path(String),
    Paths(Vec<String>),
}

/// A pass-through plugin entry the bridge appends to the catalog it writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExternalPlugin {
    pub name: String,
    pub source: ManifestExternalPluginSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<ExternalPluginSkills>,
}

/// The plugin `source` object of Claude Code's `marketplace.json`, flattened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExternalPluginSource {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
}
