use anyhow::Result;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::a2a::Artifact;
use crate::services::SkillService;
use systemprompt_models::{
    AiMessage, AiProvider, AiRequest, CallToolResult, MessageRole, RequestContext, ToolCall,
    ToolResultFormatter,
};

use super::message::StreamEvent;
use crate::models::AgentRuntimeInfo;

/// Resolves provider, model, and max_output_tokens from tool_model_config or
/// fallbacks. Priority: tool_model_config > agent_runtime > ai_service defaults
fn resolve_provider_config(
    request_context: &RequestContext,
    agent_runtime: &AgentRuntimeInfo,
    ai_service: &dyn AiProvider,
) -> (String, String, u32) {
    if let Some(config) = request_context.tool_model_config() {
        let provider = config
            .provider
            .as_deref()
            .or(agent_runtime.provider.as_deref())
            .unwrap_or_else(|| ai_service.default_provider())
            .to_string();
        let model = config
            .model
            .as_deref()
            .or(agent_runtime.model.as_deref())
            .unwrap_or_else(|| ai_service.default_model())
            .to_string();
        let max_tokens = config
            .max_output_tokens
            .or(agent_runtime.max_output_tokens)
            .unwrap_or_else(|| ai_service.default_max_output_tokens());

        tracing::debug!(
            provider,
            model,
            max_output_tokens = max_tokens,
            "Using tool_model_config"
        );

        return (provider, model, max_tokens);
    }

    let provider = agent_runtime
        .provider
        .as_deref()
        .unwrap_or_else(|| ai_service.default_provider())
        .to_string();
    let model = agent_runtime
        .model
        .as_deref()
        .unwrap_or_else(|| ai_service.default_model())
        .to_string();
    let max_tokens = agent_runtime
        .max_output_tokens
        .unwrap_or_else(|| ai_service.default_max_output_tokens());

    (provider, model, max_tokens)
}

pub async fn synthesize_tool_results_with_artifacts(
    ai_service: Arc<dyn AiProvider>,
    agent_runtime: &AgentRuntimeInfo,
    original_messages: Vec<AiMessage>,
    initial_response: &str,
    tool_calls: &[ToolCall],
    tool_results: &[CallToolResult],
    artifacts: &[Artifact],
    tx: mpsc::UnboundedSender<StreamEvent>,
    request_context: RequestContext,
    _skill_service: Arc<SkillService>,
) -> Result<String, ()> {
    let tool_results_context = ToolResultFormatter::format_for_synthesis(tool_calls, tool_results);
    let artifact_references = build_artifact_references(artifacts);

    let synthesis_prompt = build_synthesis_prompt(
        tool_calls.len(),
        &tool_results_context,
        &artifact_references,
    );

    let mut synthesis_messages = original_messages;
    synthesis_messages.push(AiMessage {
        role: MessageRole::Assistant,
        content: initial_response.to_string(),
        parts: Vec::new(),
    });
    synthesis_messages.push(AiMessage {
        role: MessageRole::User,
        content: synthesis_prompt,
        parts: Vec::new(),
    });

    tracing::info!(
        tool_result_count = tool_results.len(),
        "Calling AI to synthesize tool results"
    );

    let (provider, model, max_output_tokens) =
        resolve_provider_config(&request_context, agent_runtime, ai_service.as_ref());

    let synthesis_request = AiRequest::builder(
        synthesis_messages,
        &provider,
        &model,
        max_output_tokens,
        request_context,
    )
    .build();

    match ai_service.generate(&synthesis_request).await {
        Ok(response) => {
            let synthesized_text = response.content;

            tracing::info!(text_len = synthesized_text.len(), "Synthesis complete");

            if tx
                .send(StreamEvent::Text(synthesized_text.clone()))
                .is_err()
            {
                tracing::debug!("Stream receiver dropped during synthesis");
            }

            Ok(synthesized_text)
        },
        Err(e) => {
            tracing::error!(error = %e, "Synthesis failed");
            Err(())
        },
    }
}

pub async fn process_without_tools(
    ai_service: Arc<dyn AiProvider>,
    agent_runtime: &AgentRuntimeInfo,
    ai_messages: Vec<AiMessage>,
    tx: mpsc::UnboundedSender<StreamEvent>,
    request_context: RequestContext,
) -> Result<(String, Vec<ToolCall>, Vec<CallToolResult>), ()> {
    let (provider, model, max_output_tokens) =
        resolve_provider_config(&request_context, agent_runtime, ai_service.as_ref());

    let generate_request = AiRequest::builder(
        ai_messages,
        &provider,
        &model,
        max_output_tokens,
        request_context,
    )
    .build();

    match ai_service.generate_stream(&generate_request).await {
        Ok(mut stream) => {
            let mut accumulated_text = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(text) => {
                        accumulated_text.push_str(&text);
                        if tx.send(StreamEvent::Text(text)).is_err() {
                            tracing::debug!("Stream receiver dropped during generation");
                        }
                    },
                    Err(e) => {
                        if tx.send(StreamEvent::Error(e.to_string())).is_err() {
                            tracing::debug!("Stream receiver dropped while sending error");
                        }
                        return Err(());
                    },
                }
            }
            Ok((accumulated_text, Vec::new(), Vec::new()))
        },
        Err(e) => {
            if tx.send(StreamEvent::Error(e.to_string())).is_err() {
                tracing::debug!("Stream receiver dropped while sending error");
            }
            Err(())
        },
    }
}

fn build_synthesis_prompt(
    tool_count: usize,
    tool_results_context: &str,
    artifact_references: &str,
) -> String {
    format!(
        r#"# Tool Execution Complete

You executed {} tool(s). Now provide a BRIEF conversational response.

## Tool Results Summary

{}

## Artifacts Created

{}

## CRITICAL RULES - READ CAREFULLY

1. **NEVER repeat artifact content** - The user sees artifacts separately. Your message should REFERENCE them, never duplicate their content.
2. **Maximum 100 words** - Be extremely concise. 2-3 sentences is ideal.
3. **Describe what was done, not what it contains** - Say "I've created a blog post about X" NOT "Here is the blog post: [full content]"
4. **Be conversational** - Natural, friendly summary. Not a report or transcript.
5. **Reference artifacts naturally** - Use format like "(see the artifact for the full content)"

## BAD EXAMPLE (DO NOT DO THIS)
"I've created your blog post. Here's the content:

[2000 words of article text]

Let me know if you'd like any changes."

## GOOD EXAMPLE
"Done! I've created a blog post exploring the Human-AI collaboration workflow. The article covers the key differences between automation and augmentation approaches, with practical steps for maintaining your authentic voice. Take a look at the artifact and let me know if you'd like any adjustments."

---

Provide your brief, conversational response now. Remember: the artifact has the content - your message is just the friendly summary."#,
        tool_count, tool_results_context, artifact_references
    )
}

fn build_artifact_references(artifacts: &[Artifact]) -> String {
    if artifacts.is_empty() {
        return "No artifacts were created.".to_string();
    }

    artifacts
        .iter()
        .map(|artifact| {
            let artifact_type = &artifact.metadata.artifact_type;
            let artifact_name = artifact
                .name
                .clone()
                .unwrap_or_else(|| artifact.id.to_string());

            format!(
                "- **{}** ({}): Reference as '(see {} for details)'",
                artifact_name, artifact_type, artifact_name
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
