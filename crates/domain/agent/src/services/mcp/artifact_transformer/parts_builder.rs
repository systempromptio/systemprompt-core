use crate::error::ArtifactError;
use crate::models::a2a::{DataPart, FilePart, FileWithBytes, Part, TextPart};
use serde_json::Value as JsonValue;

pub fn build_parts(artifact: &JsonValue) -> Result<Vec<Part>, ArtifactError> {
    let mut parts = Vec::new();

    if let Some(obj) = artifact.as_object() {
        parts.push(Part::Data(DataPart { data: obj.clone() }));
        return Ok(parts);
    }

    if let Some(content) = artifact.get("content") {
        if let Some(arr) = content.as_array() {
            for item in arr {
                if let Some(content_type) = item.get("type").and_then(|t| t.as_str()) {
                    match content_type {
                        "text" => {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                parts.push(Part::Text(TextPart {
                                    text: text.to_string(),
                                }));
                            }
                        },
                        "image" => {
                            if let Some(data) = item.get("data").and_then(|d| d.as_str()) {
                                let mime_type = item
                                    .get("mimeType")
                                    .and_then(|m| m.as_str())
                                    .map(|s| s.to_string());

                                parts.push(Part::File(FilePart {
                                    file: FileWithBytes {
                                        bytes: data.to_string(),
                                        mime_type,
                                        name: None,
                                    },
                                }));
                            }
                        },
                        "resource" => {
                            if let Some(uri) = item.get("uri").and_then(|u| u.as_str()) {
                                let mime_type = item
                                    .get("mimeType")
                                    .and_then(|m| m.as_str())
                                    .map(|s| s.to_string());

                                parts.push(Part::File(FilePart {
                                    file: FileWithBytes {
                                        name: Some(uri.to_string()),
                                        mime_type,
                                        bytes: String::new(),
                                    },
                                }));
                            }
                        },
                        _ => {},
                    }
                }
            }
            if !parts.is_empty() {
                return Ok(parts);
            }
        }
    }

    Err(ArtifactError::Transform(format!(
        "Artifact must be an object or contain a 'content' array. Received: {}",
        serde_json::to_string_pretty(artifact).unwrap_or_else(|_| "invalid JSON".to_string())
    )))
}
