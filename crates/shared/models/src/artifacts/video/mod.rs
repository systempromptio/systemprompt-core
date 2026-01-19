use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use systemprompt_identifiers::SkillId;

fn default_artifact_type() -> String {
    "video".to_string()
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default = "default_true")]
    pub controls: bool,
    #[serde(default)]
    pub autoplay: bool,
    #[serde(default)]
    #[serde(rename = "loop")]
    pub loop_playback: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

impl VideoArtifact {
    pub fn new(src: impl Into<String>, ctx: &RequestContext) -> Self {
        Self {
            artifact_type: "video".to_string(),
            src: src.into(),
            mime_type: None,
            poster: None,
            caption: None,
            controls: true,
            autoplay: false,
            loop_playback: false,
            muted: false,
            metadata: ExecutionMetadata::with_request(ctx),
        }
    }

    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn with_poster(mut self, poster: impl Into<String>) -> Self {
        self.poster = Some(poster.into());
        self
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub const fn with_autoplay(mut self) -> Self {
        self.autoplay = true;
        self.muted = true;
        self
    }

    pub const fn with_loop(mut self) -> Self {
        self.loop_playback = true;
        self
    }

    pub const fn without_controls(mut self) -> Self {
        self.controls = false;
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

impl Artifact for VideoArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Video
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "src": {
                    "type": "string",
                    "description": "Video source URL or base64 data URI"
                },
                "mime_type": {
                    "type": "string",
                    "description": "MIME type (e.g., video/mp4)"
                },
                "poster": {
                    "type": "string",
                    "description": "Poster/thumbnail image URL"
                },
                "caption": {
                    "type": "string",
                    "description": "Caption displayed below the video"
                },
                "controls": {
                    "type": "boolean",
                    "description": "Show playback controls",
                    "default": true
                },
                "autoplay": {
                    "type": "boolean",
                    "description": "Auto-play on load",
                    "default": false
                },
                "loop": {
                    "type": "boolean",
                    "description": "Loop playback",
                    "default": false
                },
                "muted": {
                    "type": "boolean",
                    "description": "Mute audio",
                    "default": false
                }
            },
            "required": ["src"],
            "x-artifact-type": "video"
        })
    }
}
