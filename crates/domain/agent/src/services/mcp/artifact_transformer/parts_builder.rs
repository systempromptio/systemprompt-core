//! Builds artifact parts from transformed tool results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::ArtifactError;
use crate::models::a2a::{DataPart, Part};
use serde_json::Value as JsonValue;

// JSON: MCP tool result — structured content is schema-less per the spec.
pub fn build_parts(artifact: &JsonValue) -> Result<Vec<Part>, ArtifactError> {
    if let Some(obj) = artifact.as_object() {
        return Ok(vec![Part::Data(DataPart { data: obj.clone() })]);
    }

    Err(ArtifactError::ArtifactNotObject {
        found: json_kind(artifact),
    })
}

const fn json_kind(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "a boolean",
        JsonValue::Number(_) => "a number",
        JsonValue::String(_) => "a string",
        JsonValue::Array(_) => "an array",
        JsonValue::Object(_) => "an object",
    }
}
