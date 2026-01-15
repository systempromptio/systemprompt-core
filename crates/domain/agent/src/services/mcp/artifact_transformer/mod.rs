mod metadata_builder;
mod parts_builder;
mod type_inference;

use crate::error::ArtifactError;
use crate::models::a2a::Artifact;
use rmcp::model::CallToolResult;
use serde_json::{json, Value as JsonValue};
use systemprompt_models::artifacts::types::ArtifactType;

use metadata_builder::build_metadata;
use parts_builder::{build_parts, build_parts_from_result};
use type_inference::{infer_type, infer_type_from_result};

pub fn unwrap_tool_response(structured_content: &JsonValue) -> (&JsonValue, Option<&JsonValue>) {
    if let (Some(artifact), Some(metadata)) = (
        structured_content.get("artifact"),
        structured_content.get("_metadata"),
    ) {
        (artifact, Some(metadata))
    } else {
        (structured_content, None)
    }
}

pub fn extract_artifact_id(structured_content: &JsonValue) -> Option<String> {
    structured_content
        .get("artifact_id")
        .and_then(|v| v.as_str())
        .map(String::from)
}

pub fn extract_skill_id(structured_content: &JsonValue) -> Option<String> {
    structured_content
        .get("skill_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            structured_content
                .get("_metadata")
                .and_then(|m| m.get("skill_id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
}

pub fn extract_execution_id(structured_content: &JsonValue) -> Option<String> {
    structured_content
        .get("_metadata")
        .and_then(|m| m.get("execution_id"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

pub fn artifact_type_to_string(artifact_type: &ArtifactType) -> String {
    match artifact_type {
        ArtifactType::Text => "text".to_string(),
        ArtifactType::Table => "table".to_string(),
        ArtifactType::Chart => "chart".to_string(),
        ArtifactType::Form => "form".to_string(),
        ArtifactType::Dashboard => "dashboard".to_string(),
        ArtifactType::PresentationCard => "presentation_card".to_string(),
        ArtifactType::List => "list".to_string(),
        ArtifactType::CopyPasteText => "copy_paste_text".to_string(),
        ArtifactType::Image => "image".to_string(),
        ArtifactType::Video => "video".to_string(),
        ArtifactType::Audio => "audio".to_string(),
        ArtifactType::Custom(name) => name.clone(),
    }
}

pub fn calculate_fingerprint(tool_name: &str, tool_arguments: Option<&JsonValue>) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let args_str = tool_arguments
        .and_then(|args| serde_json::to_string(args).ok())
        .unwrap_or_default();

    let mut hasher = DefaultHasher::new();
    args_str.hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}-{:x}", tool_name, hash)
}

#[derive(Debug, Copy, Clone)]
pub struct McpToA2aTransformer;

impl McpToA2aTransformer {
    pub fn transform(
        tool_name: &str,
        tool_result: &CallToolResult,
        output_schema: Option<&JsonValue>,
        context_id: &str,
        task_id: &str,
        tool_arguments: Option<&JsonValue>,
    ) -> Result<Artifact, ArtifactError> {
        let artifact_type = infer_type_from_result(tool_result, output_schema, tool_name)?;

        let execution_id = tool_result
            .structured_content
            .as_ref()
            .and_then(extract_execution_id);

        let fingerprint = calculate_fingerprint(tool_name, tool_arguments);

        let skill_id = tool_result
            .structured_content
            .as_ref()
            .and_then(extract_skill_id);

        let parts = build_parts_from_result(tool_result)?;
        let mut metadata = build_metadata(
            &artifact_type,
            output_schema,
            execution_id,
            context_id,
            task_id,
            tool_name,
        )?;

        metadata = metadata.with_fingerprint(fingerprint);

        if let Some(sid) = skill_id {
            metadata = metadata.with_skill_id(sid);
        }

        let artifact_id = tool_result
            .structured_content
            .as_ref()
            .and_then(extract_artifact_id)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        Ok(Artifact {
            id: artifact_id.into(),
            name: Some(tool_name.to_string()),
            description: None,
            parts,
            metadata,
            extensions: vec![json!(
                "https://systemprompt.io/extensions/artifact-rendering/v1"
            )],
        })
    }

    pub fn transform_from_json(
        tool_name: &str,
        tool_result_json: &JsonValue,
        output_schema: Option<&JsonValue>,
        context_id: &str,
        task_id: &str,
        tool_arguments: Option<&JsonValue>,
    ) -> Result<Artifact, ArtifactError> {
        let artifact_type = infer_type(tool_result_json, output_schema, tool_name)?;

        let execution_id = extract_execution_id(tool_result_json);
        let fingerprint = calculate_fingerprint(tool_name, tool_arguments);
        let skill_id = extract_skill_id(tool_result_json);

        let parts = build_parts(tool_result_json)?;
        let mut metadata = build_metadata(
            &artifact_type,
            output_schema,
            execution_id,
            context_id,
            task_id,
            tool_name,
        )?;

        metadata = metadata.with_fingerprint(fingerprint);

        if let Some(sid) = skill_id {
            metadata = metadata.with_skill_id(sid);
        }

        let artifact_id = extract_artifact_id(tool_result_json)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        Ok(Artifact {
            id: artifact_id.into(),
            name: Some(tool_name.to_string()),
            description: None,
            parts,
            metadata,
            extensions: vec![json!(
                "https://systemprompt.io/extensions/artifact-rendering/v1"
            )],
        })
    }
}
