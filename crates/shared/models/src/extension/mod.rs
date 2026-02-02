use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionType {
    Mcp,
    Blog,
    Cli,
    #[default]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildType {
    #[default]
    Workspace,
    Submodule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRole {
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
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

    #[serde(default)]
    pub build_type: BuildType,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub roles: HashMap<String, ManifestRole>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CliCommand>,

    #[serde(default)]
    pub supports_json_output: bool,
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
    pub const fn new(manifest: ExtensionManifest, path: PathBuf, manifest_path: PathBuf) -> Self {
        Self {
            manifest,
            path,
            manifest_path,
        }
    }

    pub const fn extension_type(&self) -> ExtensionType {
        self.manifest.extension.type_
    }

    pub fn binary_name(&self) -> Option<&str> {
        self.manifest.extension.binary.as_deref()
    }

    pub const fn is_enabled(&self) -> bool {
        self.manifest.extension.enabled
    }

    pub fn is_mcp(&self) -> bool {
        self.manifest.extension.type_ == ExtensionType::Mcp
    }

    pub fn is_cli(&self) -> bool {
        self.manifest.extension.type_ == ExtensionType::Cli
    }

    pub fn commands(&self) -> &[CliCommand] {
        &self.manifest.extension.commands
    }

    pub const fn build_type(&self) -> BuildType {
        self.manifest.extension.build_type
    }

    pub const fn supports_json_output(&self) -> bool {
        self.manifest.extension.supports_json_output
    }
}
