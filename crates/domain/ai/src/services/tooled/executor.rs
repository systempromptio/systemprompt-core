use serde_json::json;
use std::sync::Arc;

use crate::models::tools::{CallToolResult, McpTool, ToolCall};
use crate::services::tools::{
    request_context_to_tool_context, tool_call_to_request, trait_result_to_rmcp_result,
};
use systemprompt_models::ai::ToolModelOverrides;
use systemprompt_traits::{ToolCallResult as TraitToolCallResult, ToolContent, ToolProvider};

#[derive(Debug)]
pub enum ResponseStrategy {
    ContentProvided {
        content: String,
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
    },
    ArtifactsProvided {
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
    },
    ToolsOnly {
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
    },
}

impl ResponseStrategy {
    pub fn from_response(
        content: String,
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
    ) -> Self {
        if !content.trim().is_empty() {
            Self::ContentProvided {
                content,
                tool_calls,
                tool_results,
            }
        } else if !tool_calls.is_empty() && !tool_results.is_empty() {
            if Self::has_valid_artifacts(&tool_results) {
                Self::ArtifactsProvided {
                    tool_calls,
                    tool_results,
                }
            } else {
                Self::ToolsOnly {
                    tool_calls,
                    tool_results,
                }
            }
        } else {
            Self::ContentProvided {
                content,
                tool_calls,
                tool_results,
            }
        }
    }

    fn has_valid_artifacts(tool_results: &[CallToolResult]) -> bool {
        tool_results
            .iter()
            .any(|result| result.structured_content.is_some() && result.is_error != Some(true))
    }
}

pub struct TooledExecutor {
    tool_provider: Arc<dyn ToolProvider>,
}

impl std::fmt::Debug for TooledExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TooledExecutor").finish_non_exhaustive()
    }
}

impl TooledExecutor {
    pub fn new(tool_provider: Arc<dyn ToolProvider>) -> Self {
        Self { tool_provider }
    }

    pub async fn execute_tool_calls(
        &self,
        tool_calls: Vec<ToolCall>,
        tools: &[McpTool],
        context: &systemprompt_models::RequestContext,
        agent_overrides: Option<&ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        let default_overrides = ToolModelOverrides::new();
        let overrides = agent_overrides.unwrap_or(&default_overrides);
        let mut tool_results = Vec::new();

        for tool_call in &tool_calls {
            let tool = tools.iter().find(|t| t.name == tool_call.name);

            if let Some(tool) = tool {
                let resolved_config = resolve_model_config(tool, overrides);
                let enriched_ctx = resolved_config.map_or_else(
                    || context.clone(),
                    |config| context.clone().with_tool_model_config(config),
                );

                let tool_context = request_context_to_tool_context(&enriched_ctx);
                let request = tool_call_to_request(tool_call);

                match self
                    .tool_provider
                    .call_tool(&request, tool.service_id.as_str(), &tool_context)
                    .await
                {
                    Ok(result) => {
                        tool_results.push(trait_result_to_rmcp_result(&result));
                    },
                    Err(e) => {
                        let error_result = TraitToolCallResult {
                            content: vec![ToolContent::text(format!("Error: {e}"))],
                            structured_content: Some(json!({"error": e.to_string()})),
                            is_error: Some(true),
                            meta: None,
                        };
                        tool_results.push(trait_result_to_rmcp_result(&error_result));
                    },
                }
            } else {
                let error_result = TraitToolCallResult {
                    content: vec![ToolContent::text(format!(
                        "Error: Tool '{}' not found in provided tools list",
                        tool_call.name
                    ))],
                    structured_content: Some(json!({
                        "error": format!("Tool '{}' not found", tool_call.name)
                    })),
                    is_error: Some(true),
                    meta: None,
                };
                tool_results.push(trait_result_to_rmcp_result(&error_result));
            }
        }

        (tool_calls, tool_results)
    }
}

fn resolve_model_config(
    tool: &McpTool,
    agent_overrides: &ToolModelOverrides,
) -> Option<systemprompt_models::ai::ToolModelConfig> {
    if let Some(server_overrides) = agent_overrides.get(tool.service_id.as_str()) {
        if let Some(tool_override) = server_overrides.get(&tool.name) {
            return Some(tool_override.clone());
        }
    }
    tool.model_config.clone()
}
