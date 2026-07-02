//! Cowork library-artifact on-disk descriptor.
//!
//! [`DiskArtifactConfig`] is the `services/artifacts/<id>/config.yaml` shape
//! the marketplace loader projects into a signed
//! [`crate::bridge::manifest::ArtifactEntry`]. The HTML body lives in a sibling
//! file (default `content.html`) referenced by [`DiskArtifactConfig::file`].
//! These are Cowork-native library documents, not the in-chat MCP artifacts in
//! [`crate::artifacts`].

use serde::Deserialize;

use crate::bridge::ids::{LibraryArtifactId, PluginId};

const fn default_true() -> bool {
    true
}

fn default_artifact_version() -> String {
    "1".to_owned()
}

pub const ARTIFACT_CONFIG_FILENAME: &str = "config.yaml";
pub const DEFAULT_ARTIFACT_CONTENT_FILE: &str = "content.html";

#[derive(Debug, Clone, Deserialize)]
pub struct DiskArtifactConfig {
    pub id: LibraryArtifactId,
    pub name: String,
    pub description: String,
    #[serde(default = "default_artifact_version")]
    pub version: String,
    pub plugin_id: PluginId,
    #[serde(default)]
    pub mcp_tools: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub file: String,
}

impl DiskArtifactConfig {
    pub fn content_file(&self) -> &str {
        if self.file.is_empty() {
            DEFAULT_ARTIFACT_CONTENT_FILE
        } else {
            &self.file
        }
    }
}
