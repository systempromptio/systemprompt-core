//! Text artifact: a titled block of free-form text returned by skills and
//! tools.
//!
//! [`TextArtifact`] is the builder-style producer carrying optional title and
//! execution metadata.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::artifacts::metadata::ArtifactProvenance;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::{SkillId, SkillName};

fn default_artifact_type() -> String {
    "text".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ArtifactProvenance,
}

impl TextArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "text";

    pub fn new(content: impl Into<String>) -> Self {
        Self {
            artifact_type: "text".to_owned(),
            content: content.into(),
            title: None,
            metadata: ArtifactProvenance::default(),
        }
    }

    pub fn with_request(mut self, ctx: &RequestContext) -> Self {
        self.metadata.set_request(ctx);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        self.metadata.set_execution_id(id);
        self
    }

    pub fn with_skill(mut self, skill_id: SkillId, skill_name: SkillName) -> Self {
        self.metadata.set_skill(skill_id, skill_name);
        self
    }
}

impl Artifact for TextArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Text
    }

    // JSON: JSON Schema document describing the artifact for the model.
    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Text content"
                },
                "title": {
                    "type": "string",
                    "description": "Optional title for the text"
                }
            },
            "required": ["content"],
            "x-artifact-type": "text"
        })
    }
}
