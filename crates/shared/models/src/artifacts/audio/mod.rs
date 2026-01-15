use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

fn default_artifact_type() -> String {
    "audio".to_string()
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artwork: Option<String>,
    #[serde(default = "default_true")]
    pub controls: bool,
    #[serde(default)]
    pub autoplay: bool,
    #[serde(default)]
    #[serde(rename = "loop")]
    pub loop_playback: bool,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

impl AudioArtifact {
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            artifact_type: "audio".to_string(),
            src: src.into(),
            mime_type: None,
            title: None,
            artist: None,
            artwork: None,
            controls: true,
            autoplay: false,
            loop_playback: false,
            metadata: ExecutionMetadata::default(),
        }
    }

    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    pub fn with_artwork(mut self, artwork: impl Into<String>) -> Self {
        self.artwork = Some(artwork.into());
        self
    }

    pub const fn with_autoplay(mut self) -> Self {
        self.autoplay = true;
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

    pub fn with_execution_id(mut self, id: String) -> Self {
        self.metadata.execution_id = Some(id);
        self
    }

    pub fn with_skill(
        mut self,
        skill_id: impl Into<String>,
        skill_name: impl Into<String>,
    ) -> Self {
        self.metadata = self.metadata.with_skill(skill_id.into(), skill_name.into());
        self
    }
}

impl Artifact for AudioArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Audio
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "src": {
                    "type": "string",
                    "description": "Audio source URL or base64 data URI"
                },
                "mime_type": {
                    "type": "string",
                    "description": "MIME type (e.g., audio/mpeg)"
                },
                "title": {
                    "type": "string",
                    "description": "Track title"
                },
                "artist": {
                    "type": "string",
                    "description": "Artist name"
                },
                "artwork": {
                    "type": "string",
                    "description": "Album artwork URL"
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
                }
            },
            "required": ["src"],
            "x-artifact-type": "audio"
        })
    }
}
