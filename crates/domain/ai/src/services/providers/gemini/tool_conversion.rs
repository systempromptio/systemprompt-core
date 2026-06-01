//! Gemini tool-schema transformation and tool-call name resolution.
//!
//! Gemini rejects several JSON-Schema constructs and flattens discriminated
//! unions, so tool schemas are run through the [`SchemaTransformer`] and the
//! resulting name rewrites are tracked in the per-provider [`ToolNameMapper`].
//! On the way back, model-emitted function names are resolved to their original
//! tool names. This is the agent-only concern the shared codec does not carry.

use std::collections::HashSet;

use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalTool, ThinkingConfig,
};
use uuid::Uuid;

use crate::error::Result;
use crate::models::tools::{McpTool, ToolCall};
use crate::services::schema::{DiscriminatedUnion, ProviderCapabilities, SchemaTransformer};

use super::constants::tokens;
use super::provider::GeminiProvider;

pub(super) async fn convert_tools(
    provider: &GeminiProvider,
    tools: Vec<McpTool>,
) -> Result<Vec<CanonicalTool>> {
    let transformer = SchemaTransformer::new(ProviderCapabilities::gemini());
    let mut mapper = provider.tool_mapper.lock().await;
    let mut seen_names = HashSet::new();

    let transformed = tools
        .into_iter()
        .map(|tool| {
            let discriminator_field = tool
                .input_schema
                .as_ref()
                .and_then(DiscriminatedUnion::detect)
                .map(|u| u.discriminator_field);
            let result = transformer.transform(&tool)?;
            for t in &result {
                mapper.register_transformation(t, discriminator_field.clone());
            }
            Ok(result)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .filter(|tool| seen_names.insert(tool.name.clone()))
        .map(|tool| CanonicalTool {
            name: tool.name,
            description: Some(tool.description),
            input_schema: tool.input_schema,
        })
        .collect();

    Ok(transformed)
}

pub(super) async fn resolve_response(
    provider: &GeminiProvider,
    response: &CanonicalResponse,
) -> (String, Vec<ToolCall>) {
    let mapper = provider.tool_mapper.lock().await;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for part in &response.content {
        match part {
            CanonicalContent::Text(text) => content.push_str(text),
            CanonicalContent::ToolUse { name, input, .. } => {
                let (original_name, resolved_args) = mapper.resolve_tool_call(name, input.clone());
                tool_calls.push(ToolCall {
                    ai_tool_call_id: AiToolCallId::new(Uuid::new_v4().to_string()),
                    name: original_name,
                    arguments: resolved_args,
                });
            },
            _ => {},
        }
    }
    (content, tool_calls)
}

pub(super) fn thinking_for(model: &str) -> Option<ThinkingConfig> {
    if model.contains("2.5") {
        Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(tokens::THINKING_BUDGET),
        })
    } else {
        None
    }
}
