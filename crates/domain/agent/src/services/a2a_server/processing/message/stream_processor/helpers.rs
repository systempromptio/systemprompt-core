use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::a2a::Artifact;
use crate::models::AgentRuntimeInfo;
use crate::services::a2a_server::processing::artifact::ArtifactBuilder;
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::SkillService;
use systemprompt_models::{AiMessage, AiProvider, RequestContext};

pub fn build_artifacts_from_results(
    tool_results: &[systemprompt_models::CallToolResult],
    tool_calls: &[systemprompt_models::ToolCall],
    tools: &[systemprompt_models::McpTool],
    context_id_str: &str,
    task_id_str: &str,
) -> Result<Vec<Artifact>> {
    if tool_results.is_empty() {
        tracing::info!("No tool_results - no artifacts expected");
        return Ok(Vec::new());
    }

    let has_structured_content = tool_results.iter().any(|r| r.structured_content.is_some());

    if !has_structured_content {
        tracing::info!(
            "No structured_content - ephemeral tool calls, skipping A2A artifact building"
        );
        return Ok(Vec::new());
    }

    tracing::info!(
        "Tool results contain structured_content - building A2A artifacts from agentic MCP calls"
    );

    let artifact_builder = ArtifactBuilder::new(
        tool_calls.to_vec(),
        tool_results.to_vec(),
        tools.to_vec(),
        context_id_str.to_string(),
        task_id_str.to_string(),
    );

    artifact_builder.build_artifacts()
}

pub async fn synthesize_final_response(
    tool_calls: &[systemprompt_models::ToolCall],
    tool_results: &[systemprompt_models::CallToolResult],
    artifacts: &[Artifact],
    accumulated_text: &str,
    ai_service: Arc<dyn AiProvider>,
    agent_runtime: &AgentRuntimeInfo,
    ai_messages_for_synthesis: Vec<AiMessage>,
    tx: mpsc::UnboundedSender<StreamEvent>,
    request_ctx: RequestContext,
    skill_service: Arc<SkillService>,
) -> String {
    use crate::services::a2a_server::processing::ai_executor::synthesize_tool_results_with_artifacts;

    if !tool_calls.is_empty() && !tool_results.is_empty() {
        tracing::info!(
            tool_call_count = tool_calls.len(),
            artifact_count = artifacts.len(),
            "Synthesizing results from tool calls"
        );

        match synthesize_tool_results_with_artifacts(
            ai_service,
            agent_runtime,
            ai_messages_for_synthesis,
            accumulated_text,
            tool_calls,
            tool_results,
            artifacts,
            tx,
            request_ctx,
            skill_service,
        )
        .await
        {
            Ok(synthesized) => synthesized,
            Err(_) => {
                tracing::warn!("Synthesis failed, using initial response");
                accumulated_text.to_string()
            },
        }
    } else {
        if tool_calls.is_empty() && !accumulated_text.is_empty() {
            tracing::warn!(
                response_len = accumulated_text.len(),
                "Synthesis skipped: Agent produced text without tool calls"
            );
        }
        accumulated_text.to_string()
    }
}
