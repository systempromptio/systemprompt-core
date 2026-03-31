mod metadata_builder;
mod parts_builder;
mod type_inference;

use crate::error::{ArtifactError, RowParseError};
use crate::models::a2a::Artifact;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::artifacts::types::ArtifactType;

use metadata_builder::{BuildMetadataParams, build_metadata};
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
        return Err(RowParseError::MissingField {
            field: "structured_content (received null)".to_string(),
        }
        .into());
    }

    if let Some(obj) = structured_content.as_object() {
        if obj.is_empty() {
            return Err(RowParseError::MissingField {
                field: "structured_content (received empty object {})".to_string(),
            }
            .into());
        }
    }

    serde_json::from_value(structured_content.clone()).map_err(|e| {
        let actual_keys = structured_content
            .as_object()
            .map_or_else(Vec::new, |o| o.keys().cloned().collect());
        ArtifactError::InvalidSchema {
            expected: "ToolResponse {artifact_id, mcp_execution_id, artifact, _metadata}",
            actual_keys,
            source: e,
        }
    })
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

    #[allow(clippy::collection_is_never_read)]
    let args_str = tool_arguments
        .and_then(|args| {
            serde_json::to_string(args)
                .map_err(|e| {
                    tracing::debug!(error = %e, "Failed to serialize tool arguments for fingerprint");
                    e
                })
                .ok()
        })
        .unwrap_or_else(String::new);

    let mut hasher = DefaultHasher::new();
    args_str.hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}-{:x}", tool_name, hash)
}

struct TransformParsedParams<'a> {
    tool_name: &'a str,
    parsed: ParsedToolResponse,
    output_schema: Option<&'a JsonValue>,
    context_id: &'a str,
    task_id: &'a str,
    tool_arguments: Option<&'a JsonValue>,
}

fn transform_parsed(params: TransformParsedParams<'_>) -> Result<Artifact, ArtifactError> {
    let TransformParsedParams {
        tool_name,
        parsed,
        output_schema,
        context_id,
        task_id,
        tool_arguments,
    } = params;
    let artifact_type = infer_type(&parsed.artifact, output_schema, tool_name)?;
    let fingerprint = calculate_fingerprint(tool_name, tool_arguments);
    let parts = build_parts(&parsed.artifact)?;

    let mcp_execution_id = Some(parsed.mcp_execution_id.to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| parsed.metadata.execution_id.clone());

    let mut metadata = build_metadata(BuildMetadataParams {
        artifact_type: &artifact_type,
        schema: output_schema,
        mcp_execution_id,
        context_id,
        task_id,
        tool_name,
    })?;

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

#[derive(Debug)]
pub struct TransformParams<'a> {
    pub tool_name: &'a str,
    pub tool_result: &'a CallToolResult,
    pub output_schema: Option<&'a JsonValue>,
    pub context_id: &'a str,
    pub task_id: &'a str,
    pub tool_arguments: Option<&'a JsonValue>,
}

#[derive(Debug)]
pub struct TransformFromJsonParams<'a> {
    pub tool_name: &'a str,
    pub tool_result_json: &'a JsonValue,
    pub output_schema: Option<&'a JsonValue>,
    pub context_id: &'a str,
    pub task_id: &'a str,
    pub tool_arguments: Option<&'a JsonValue>,
}

#[derive(Debug, Copy, Clone)]
pub struct McpToA2aTransformer;

impl McpToA2aTransformer {
    pub fn transform(params: &TransformParams<'_>) -> Result<Artifact, ArtifactError> {
        let TransformParams {
            tool_name,
            tool_result,
            output_schema,
            context_id,
            task_id,
            tool_arguments,
        } = params;
        let structured_content =
            tool_result
                .structured_content
                .as_ref()
                .ok_or_else(|| RowParseError::MissingField {
                    field: "structured_content".to_string(),
                })?;

        let parsed = parse_tool_response(structured_content)?;
        transform_parsed(TransformParsedParams {
            tool_name,
            parsed,
            output_schema: *output_schema,
            context_id,
            task_id,
            tool_arguments: *tool_arguments,
        })
    }

    pub fn transform_from_json(
        params: &TransformFromJsonParams<'_>,
    ) -> Result<Artifact, ArtifactError> {
        let TransformFromJsonParams {
            tool_name,
            tool_result_json,
            output_schema,
            context_id,
            task_id,
            tool_arguments,
        } = params;
        let parsed = parse_tool_response(tool_result_json)?;
        transform_parsed(TransformParsedParams {
            tool_name,
            parsed,
            output_schema: *output_schema,
            context_id,
            task_id,
            tool_arguments: *tool_arguments,
        })
    }
}
