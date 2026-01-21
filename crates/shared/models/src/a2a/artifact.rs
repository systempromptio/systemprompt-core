use super::artifact_metadata::ArtifactMetadata;
use super::message::Part;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ArtifactId;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    #[serde(rename = "artifactId")]
    pub id: ArtifactId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub parts: Vec<Part>,
    pub extensions: Vec<serde_json::Value>,
    pub metadata: ArtifactMetadata,
}
