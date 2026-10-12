//! A2A artifact wire type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::artifact_metadata::ArtifactMetadata;
use super::message::Part;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ArtifactId;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    #[serde(rename = "artifactId")]
    pub id: ArtifactId,
    #[serde(default, rename = "name", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    pub metadata: ArtifactMetadata,
}
