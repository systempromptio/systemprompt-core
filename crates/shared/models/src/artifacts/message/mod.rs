//! Message/notice artifact: levelled notice lines a command emits as structured
//! output so machine consumers never receive an empty response.

use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

fn default_artifact_type() -> String {
    "message".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoticeLine {
    pub level: String,
    pub text: String,
}

impl NoticeLine {
    pub fn new(level: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            level: level.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub messages: Vec<NoticeLine>,
}

impl MessageArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "message";

    pub fn new(messages: Vec<NoticeLine>) -> Self {
        Self {
            artifact_type: "message".to_owned(),
            messages,
        }
    }
}

impl Artifact for MessageArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Custom("message".to_owned())
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "messages": {
                    "type": "array",
                    "description": "Levelled notice lines",
                    "items": {
                        "type": "object",
                        "properties": {
                            "level": {
                                "type": "string",
                                "description": "Notice level: info, success, warning, or error"
                            },
                            "text": {
                                "type": "string",
                                "description": "Notice text"
                            }
                        },
                        "required": ["level", "text"]
                    }
                }
            },
            "required": ["messages"],
            "x-artifact-type": "message"
        })
    }
}
