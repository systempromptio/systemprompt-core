//! Transformation of MCP tool results into renderable A2A [`Artifact`]s.
//!
//! [`McpToA2aTransformer`] parses a tool's `structured_content` (or raw JSON),
//! infers the
//! [`ArtifactType`](systemprompt_models::artifacts::types::ArtifactType),
//! builds parts and rendering metadata, and computes a stable fingerprint over
//! the tool name and arguments. The `metadata_builder`, `parts_builder`, and
//! `type_inference` submodules supply the metadata, part, and type-inference
//! logic.

pub mod metadata_builder;
pub mod parts_builder;
mod type_inference;

use crate::error::{ArtifactError, RowParseError};
use crate::models::a2a::Artifact;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::{ArtifactId, McpExecutionId, SkillId};

pub use metadata_builder::{BuildMetadataParams, build_metadata};
pub use parts_builder::build_parts;

pub use type_inference::infer_type;

#[derive(Debug, Deserialize)]
pub struct ParsedMetadata {
    pub skill_id: Option<SkillId>,
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
            field: "structured_content (received null)".to_owned(),
        }
        .into());
    }

    if let Some(obj) = structured_content.as_object()
        && obj.is_empty()
    {
        return Err(RowParseError::MissingField {
            field: "structured_content (received empty object {})".to_owned(),
        }
        .into());
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

pub fn calculate_fingerprint(tool_name: &str, tool_arguments: Option<&JsonValue>) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[expect(clippy::collection_is_never_read, reason = "collection is accumulated for its side-effect on the borrow checker")]
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
        title: Some(tool_name.to_owned()),
        description: None,
        parts,
        metadata,
        extensions: vec![json!(systemprompt_models::a2a::ARTIFACT_RENDERING_URI)],
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
                    field: "structured_content".to_owned(),
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
