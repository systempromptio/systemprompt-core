use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use systemprompt_identifiers::SkillId;

fn default_artifact_type() -> String {
    "image".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

impl ImageArtifact {
    pub fn new(src: impl Into<String>, ctx: &RequestContext) -> Self {
        Self {
            artifact_type: "image".to_string(),
            src: src.into(),
            alt: None,
            caption: None,
            width: None,
            height: None,
            metadata: ExecutionMetadata::with_request(ctx),
        }
    }

    pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
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

impl Artifact for ImageArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Image
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "src": {
                    "type": "string",
                    "description": "Image source URL or base64 data URI"
                },
                "alt": {
                    "type": "string",
                    "description": "Alt text for accessibility"
                },
                "caption": {
                    "type": "string",
                    "description": "Caption displayed below the image"
                },
                "width": {
                    "type": "integer",
                    "description": "Image width in pixels"
                },
                "height": {
                    "type": "integer",
                    "description": "Image height in pixels"
                }
            },
            "required": ["src"],
            "x-artifact-type": "image"
        })
    }
}
