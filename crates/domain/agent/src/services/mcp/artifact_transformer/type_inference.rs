use crate::error::ArtifactError;
use serde_json::{json, Value as JsonValue};
use systemprompt_models::artifacts::types::ArtifactType;

pub fn infer_type(
    artifact: &JsonValue,
    schema: Option<&JsonValue>,
    tool_name: &str,
) -> Result<ArtifactType, ArtifactError> {
    if let Some(schema) = schema {
        if let Some(artifact_type) = extract_artifact_type_from_schema(schema) {
            return Ok(parse_artifact_type(&artifact_type));
        }

        if is_tabular_schema(schema) {
            return Ok(ArtifactType::Table);
        }
        if is_form_schema(schema) {
            return Ok(ArtifactType::Form);
        }
        if is_chart_schema(schema) {
            return Ok(ArtifactType::Chart);
        }
    }

    if let Some(artifact_type) = extract_artifact_type_from_data(artifact) {
        return Ok(parse_artifact_type(&artifact_type));
    }

    if is_tabular_data(artifact) {
        return Ok(ArtifactType::Table);
    }

    Err(ArtifactError::Transform(format!(
        "Tool '{}' missing required x-artifact-type. Add x-artifact-type to tool output or schema.",
        tool_name
    )))
}

fn parse_artifact_type(type_str: &str) -> ArtifactType {
    match type_str.to_lowercase().as_str() {
        "text" => ArtifactType::Text,
        "table" => ArtifactType::Table,
        "chart" => ArtifactType::Chart,
        "form" => ArtifactType::Form,
        "dashboard" => ArtifactType::Dashboard,
        "presentation_card" => ArtifactType::PresentationCard,
        "list" => ArtifactType::List,
        "copy_paste_text" => ArtifactType::CopyPasteText,
        "image" => ArtifactType::Image,
        "video" => ArtifactType::Video,
        "audio" => ArtifactType::Audio,
        custom => ArtifactType::Custom(custom.to_string()),
    }
}

fn extract_artifact_type_from_data(data: &JsonValue) -> Option<String> {
    if let Some(t) = data.get("x-artifact-type").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }

    if let Some(artifact) = data.get("artifact") {
        if let Some(t) = artifact.get("x-artifact-type").and_then(|v| v.as_str()) {
            return Some(t.to_string());
        }
        if let Some(card) = artifact.get("card") {
            if let Some(t) = card.get("x-artifact-type").and_then(|v| v.as_str()) {
                return Some(t.to_string());
            }
        }
    }

    None
}

fn extract_artifact_type_from_schema(schema: &JsonValue) -> Option<String> {
    if let Some(t) = schema.get("x-artifact-type").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }

    schema
        .get("properties")
        .and_then(|props| props.get("artifact"))
        .and_then(|artifact| artifact.get("x-artifact-type"))
        .and_then(|t| t.as_str())
        .map(String::from)
}

fn is_tabular_schema(schema: &JsonValue) -> bool {
    schema.get("type") == Some(&json!("array"))
        && schema.get("items").and_then(|i| i.get("type")) == Some(&json!("object"))
}

fn is_form_schema(schema: &JsonValue) -> bool {
    if let Some(props) = schema.get("properties") {
        if let Some(fields) = props.get("fields") {
            return fields.get("type") == Some(&json!("array"));
        }
    }
    false
}

fn is_chart_schema(schema: &JsonValue) -> bool {
    if let Some(props) = schema.get("properties") {
        let has_labels = props.get("labels").is_some();
        let has_datasets = props.get("datasets").is_some();
        return has_labels && has_datasets;
    }
    false
}

fn is_tabular_data(data: &JsonValue) -> bool {
    if let Some(arr) = data.as_array() {
        if let Some(first) = arr.first() {
            return first.is_object();
        }
    }
    false
}
