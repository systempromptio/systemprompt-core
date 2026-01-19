mod metadata_builder;
mod parts_builder;
mod type_inference;

use crate::error::ArtifactError;
use crate::models::a2a::Artifact;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::artifacts::types::ArtifactType;

use metadata_builder::build_metadata;
use parts_builder::build_parts;

pub use type_inference::infer_type;

#[derive(Debug, Deserialize)]
pub struct ParsedMetadata {
    pub skill_id: Option<String>,
    pub skill_name: Option<String>,
    pub execution_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ParsedToolResponse {
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub artifact: JsonValue,
    #[serde(rename = "_metadata")]
    pub metadata: ParsedMetadata,
}

pub fn parse_tool_response(
    structured_content: &JsonValue,
) -> Result<ParsedToolResponse, ArtifactError> {
    if structured_content.is_null() {
        return Err(ArtifactError::MissingField {
            field: "structured_content (received null)".to_string(),
        });
    }

    if let Some(obj) = structured_content.as_object() {
        if obj.is_empty() {
            return Err(ArtifactError::MissingField {
                field: "structured_content (received empty object {})".to_string(),
            });
        }
    }

    serde_json::from_value(structured_content.clone()).map_err(|e| {
        let actual_keys = structured_content
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        ArtifactError::InvalidSchema {
            expected: "ToolResponse {artifact_id, mcp_execution_id, artifact, _metadata}",
            actual_keys,
            source: e,
        }
    })
}

pub(super) fn unwrap_tool_response(
    structured_content: &JsonValue,
) -> (&JsonValue, Option<&JsonValue>) {
    if let (Some(artifact), Some(metadata)) = (
        structured_content.get("artifact"),
        structured_content.get("_metadata"),
    ) {
        (artifact, Some(metadata))
    } else {
        (structured_content, None)
    }
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
        let structured_content =
            tool_result
                .structured_content
                .as_ref()
                .ok_or_else(|| ArtifactError::MissingField {
                    field: "structured_content".to_string(),
                })?;

        let parsed = parse_tool_response(structured_content)?;

        let artifact_type = infer_type(&parsed.artifact, output_schema, tool_name)?;
        let fingerprint = calculate_fingerprint(tool_name, tool_arguments);
        let parts = build_parts(&parsed.artifact)?;

        let mcp_execution_id = Some(parsed.mcp_execution_id.to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| parsed.metadata.execution_id.clone());

        let mut metadata = build_metadata(
            &artifact_type,
            output_schema,
            mcp_execution_id,
            context_id,
            task_id,
            tool_name,
        )?;

        metadata = metadata.with_fingerprint(fingerprint);

        if let Some(sid) = &parsed.metadata.skill_id {
            metadata = metadata.with_skill_id(sid.clone());
        }

        Ok(Artifact {
            id: parsed.artifact_id,
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
        let parsed = parse_tool_response(tool_result_json)?;

        let artifact_type = infer_type(&parsed.artifact, output_schema, tool_name)?;
        let fingerprint = calculate_fingerprint(tool_name, tool_arguments);
        let parts = build_parts(&parsed.artifact)?;

        let mcp_execution_id = Some(parsed.mcp_execution_id.to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| parsed.metadata.execution_id.clone());

        let mut metadata = build_metadata(
            &artifact_type,
            output_schema,
            mcp_execution_id,
            context_id,
            task_id,
            tool_name,
        )?;

        metadata = metadata.with_fingerprint(fingerprint);

        if let Some(sid) = &parsed.metadata.skill_id {
            metadata = metadata.with_skill_id(sid.clone());
        }

        Ok(Artifact {
            id: parsed.artifact_id,
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
