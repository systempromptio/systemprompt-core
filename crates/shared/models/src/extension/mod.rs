//! Extension manifest models for MCP and other extensions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {
    Mcp,
    Blog,
    #[default]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub extension: Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    #[serde(rename = "type")]
    pub type_: ExtensionType,

    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,

    #[serde(default)]
    pub description: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    #[serde(default = "default_true")]
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct DiscoveredExtension {
    pub manifest: ExtensionManifest,
    pub path: PathBuf,
    pub manifest_path: PathBuf,
}

impl DiscoveredExtension {
    pub fn new(manifest: ExtensionManifest, path: PathBuf, manifest_path: PathBuf) -> Self {
        Self {
            manifest,
            path,
            manifest_path,
        }
    }

    pub fn extension_type(&self) -> ExtensionType {
        self.manifest.extension.type_
    }

    pub fn binary_name(&self) -> Option<&str> {
        self.manifest.extension.binary.as_deref()
    }

    pub fn is_enabled(&self) -> bool {
        self.manifest.extension.enabled
    }

    pub fn is_mcp(&self) -> bool {
        self.manifest.extension.type_ == ExtensionType::Mcp
    }
}
