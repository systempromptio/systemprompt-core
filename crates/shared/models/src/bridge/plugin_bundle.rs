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
//! The manifest is also an *inbound* shape: the importer reads manifests
//! authored for Claude Code, which permit component keys systemprompt derives
//! from the tree instead (`skills`, `agents`, `commands`) and inline MCP server
//! definitions. Those keys are accepted as opaque `serde_json::Value` — an
//! external-format boundary whose shape Anthropic owns — and are never
//! serialised back out, so the bundle contract this crate emits is unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::services::PluginDependency;

pub const PLUGIN_MANIFEST_RELPATH: &str = ".claude-plugin/plugin.json";

pub const PLUGIN_MANIFEST_DIRS: &[&str] = &[".claude-plugin", "claude-plugin"];

pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";

pub const NODE_PACKAGE_FILE: &str = "package.json";

// Why: Claude Code runs a frozen, script-less install only for these
// lockfiles, checked in this order; yarn and pnpm lockfiles are skipped
// because their installers cannot be told to ignore lifecycle scripts.
pub const NODE_LOCKFILES: [&str; 4] = [
    "bun.lock",
    "bun.lockb",
    "npm-shrinkwrap.json",
    "package-lock.json",
];

#[must_use]
pub fn node_lockfile(plugin_dir: &std::path::Path) -> Option<&'static str> {
    NODE_LOCKFILES
        .iter()
        .copied()
        .find(|name| plugin_dir.join(name).is_file())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: Claude plugin manifest importer keys; Claude Code owns the schema.
    pub skills: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: Claude plugin manifest importer keys; Claude Code owns the schema.
    pub agents: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: Claude plugin manifest importer keys; Claude Code owns the schema.
    pub commands: Option<serde_json::Value>,

    #[serde(
        default,
        rename = "mcpServers",
        alias = "mcp_servers",
        skip_serializing_if = "Option::is_none"
    )]
    // JSON: Claude plugin manifest importer keys; Claude Code owns the schema.
    pub mcp_servers: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ManifestDependency>,
}

/// One `dependencies` entry in Claude Code's `plugin.json` vocabulary: a bare
/// plugin name resolved in the same marketplace, or an object naming the
/// marketplace and a semver range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManifestDependency {
    Name(String),
    Detailed {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        marketplace: Option<String>,
    },
}

impl ManifestDependency {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Name(name) | Self::Detailed { name, .. } => name,
        }
    }

    #[must_use]
    pub fn marketplace(&self) -> Option<&str> {
        match self {
            Self::Name(_) => None,
            Self::Detailed { marketplace, .. } => marketplace.as_deref(),
        }
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Name(_) => None,
            Self::Detailed { version, .. } => version.as_deref(),
        }
    }
}

impl From<&PluginDependency> for ManifestDependency {
    fn from(dependency: &PluginDependency) -> Self {
        if dependency.marketplace.is_none() && dependency.version.is_none() {
            Self::Name(dependency.name.clone())
        } else {
            Self::Detailed {
                name: dependency.name.clone(),
                version: dependency.version.clone(),
                marketplace: dependency.marketplace.clone(),
            }
        }
    }
}

impl From<&ManifestDependency> for PluginDependency {
    fn from(dependency: &ManifestDependency) -> Self {
        Self {
            name: dependency.name().to_owned(),
            marketplace: dependency.marketplace().map(str::to_owned),
            version: dependency.version().map(str::to_owned),
        }
    }
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
