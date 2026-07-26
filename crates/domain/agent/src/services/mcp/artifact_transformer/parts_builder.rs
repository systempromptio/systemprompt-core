//! Builds artifact parts from transformed tool results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::ArtifactError;
use crate::models::a2a::{DataPart, Part};
use serde_json::Value as JsonValue;

pub fn build_parts(artifact: &JsonValue) -> Result<Vec<Part>, ArtifactError> {
    if let Some(obj) = artifact.as_object() {
        return Ok(vec![Part::Data(DataPart { data: obj.clone() })]);
    }

    Err(ArtifactError::Transform(format!(
        "Artifact must be an object. Received: {}",
        serde_json::to_string_pretty(artifact).unwrap_or_else(|_| "invalid JSON".to_owned())
    )))
}
