//! Plugin bundle contract: the `.claude-plugin/plugin.json` manifest shape and
//! the well-formedness predicate every consumer shares.
//!
//! A *plugin bundle* is the installable artifact a host (Claude Code / Cowork)
//! reads: a directory rooted on `.claude-plugin/plugin.json` plus the component
//! files it ships (`skills/<n>/SKILL.md`, `agents/<n>.md`, `.mcp.json`, …).
//! [`PluginManifest`] is that manifest; [`bundle_has_manifest`] is the single
//! definition of "is this directory a well-formed bundle?" so the gateway
//! serve path, the bridge sync, the CLI generator, and the marketplace export
//! never drift on the contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const PLUGIN_MANIFEST_RELPATH: &str = ".claude-plugin/plugin.json";

/// Manifest directory names accepted on a host, in preference order.
///
/// Cowork historically materialised the manifest under both the dotted and the
/// undotted directory, so both are honoured when probing an installed bundle.
pub const PLUGIN_MANIFEST_DIRS: &[&str] = &[".claude-plugin", "claude-plugin"];

pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ManifestAuthor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(
        default,
        rename = "installationPreference",
        skip_serializing_if = "Option::is_none"
    )]
    pub installation_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAuthor {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub email: String,
}

pub fn bundle_has_manifest<S: AsRef<str>>(paths: impl IntoIterator<Item = S>) -> bool {
    paths
        .into_iter()
        .any(|path| path.as_ref() == PLUGIN_MANIFEST_RELPATH)
}
