//! Traits artifacts implement to expose their type, schema, and JSON form.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::Value as JsonValue;

use super::types::ArtifactType;

pub trait Artifact: Serialize {
    fn artifact_type(&self) -> ArtifactType;
    // JSON: JSON Schema document describing the artifact for the model.
    fn to_schema(&self) -> JsonValue;

    // JSON: Artifact rendered as the A2A `DataPart.data` object.
    fn to_json_value(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::to_value(self)
    }
}

pub trait ArtifactSchema {
    // JSON: JSON Schema document describing the artifact for the model.
    fn generate_schema(&self) -> JsonValue;
}
