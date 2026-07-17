//! Copy-to-clipboard text artifact.
//!
//! A [`CopyPasteTextArtifact`] presents a block of text intended for one-click
//! copying, with an optional title and language hint for syntax highlighting.
//! It implements [`Artifact`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::SkillId;

fn default_artifact_type() -> String {
    "copy_paste_text".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CopyPasteTextArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

impl CopyPasteTextArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "copy_paste_text";

    pub fn new(content: impl Into<String>) -> Self {
        Self {
            artifact_type: "copy_paste_text".to_owned(),
            content: content.into(),
            title: None,
            language: None,
            metadata: ExecutionMetadata::default(),
        }
    }

    pub fn with_request(mut self, ctx: &RequestContext) -> Self {
        self.metadata = ExecutionMetadata::with_request(ctx);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        self.metadata.execution_id = Some(id.into());
        self
    }

    pub fn with_skill(
        mut self,
        skill_id: impl Into<SkillId>,
        skill_name: impl Into<String>,
    ) -> Self {
        self.metadata.skill_id = Some(skill_id.into());
        self.metadata.skill_name = Some(skill_name.into());
        self
    }
}

impl Artifact for CopyPasteTextArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::CopyPasteText
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Text content to be copied"
                },
                "title": {
                    "type": "string",
                    "description": "Optional title for the content"
                },
                "language": {
                    "type": "string",
                    "description": "Optional language for syntax highlighting"
                }
            },
            "required": ["content"],
            "x-artifact-type": "copy_paste_text"
        })
    }
}
