//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::Value as JsonValue;

use super::types::ArtifactType;

pub trait Artifact: Serialize {
    fn artifact_type(&self) -> ArtifactType;
    fn to_schema(&self) -> JsonValue;

    fn to_json_value(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::to_value(self)
    }
}

pub trait ArtifactSchema {
    fn generate_schema(&self) -> JsonValue;
}
