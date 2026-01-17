use anyhow::{anyhow, Result};
use base64::Engine;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::StreamEvent;
use crate::models::a2a::{Artifact, FilePart, Message, Part};
use crate::models::AgentRuntimeInfo;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::a2a_server::processing::artifact::ArtifactBuilder;
use crate::services::a2a_server::processing::strategies::{
    ExecutionContext, ExecutionStrategySelector,
};
use crate::services::{ContextService, SkillService};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{AgentName, TaskId};
use systemprompt_models::{
    is_supported_audio, is_supported_image, is_supported_text, is_supported_video, AiContentPart,
    AiMessage, AiProvider, MessageRole, RequestContext,
};

#[allow(missing_debug_implementations)]
pub struct StreamProcessor {
    pub ai_service: Arc<dyn AiProvider>,
    pub context_service: ContextService,
    pub skill_service: Arc<SkillService>,
    pub execution_step_repo: Arc<ExecutionStepRepository>,
    pub db_pool: DbPool,
}

impl StreamProcessor {
    pub fn extract_message_text(message: &Message) -> Result<String> {
        for part in &message.parts {
            if let Part::Text(text_part) = part {
                return Ok(text_part.text.clone());
            }
        }
        Err(anyhow!("No text content found in message"))
    }

    pub fn extract_message_content(message: &Message) -> (String, Vec<AiContentPart>) {
        let mut text_content = String::new();
        let mut content_parts = Vec::new();

        for part in &message.parts {
            match part {
                Part::Text(text_part) => {
                    if text_content.is_empty() {
                        text_content.clone_from(&text_part.text);
                    }
                    content_parts.push(AiContentPart::text(&text_part.text));
                },
                Part::File(file_part) => {
                    if let Some(content_part) = Self::file_to_content_part(file_part) {
                        content_parts.push(content_part);
                    }
                },
                Part::Data(_) => {},
            }
        }

        (text_content, content_parts)
    }

    fn file_to_content_part(file_part: &FilePart) -> Option<AiContentPart> {
        let mime_type = file_part.file.mime_type.as_deref()?;
        let file_name = file_part.file.name.as_deref().unwrap_or("unnamed");

        if is_supported_image(mime_type) {
            return Some(AiContentPart::image(mime_type, &file_part.file.bytes));
        }

        if is_supported_audio(mime_type) {
            return Some(AiContentPart::audio(mime_type, &file_part.file.bytes));
        }

        if is_supported_video(mime_type) {
            return Some(AiContentPart::video(mime_type, &file_part.file.bytes));
        }

        if is_supported_text(mime_type) {
            return Self::decode_text_file(file_part, file_name, mime_type);
        }

        tracing::warn!(
            file_name = %file_name,
            mime_type = %mime_type,
            "Unsupported file type - file will not be sent to AI"
        );
        None
    }

    fn decode_text_file(
        file_part: &FilePart,
        file_name: &str,
        mime_type: &str,
    ) -> Option<AiContentPart> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&file_part.file.bytes)
            .map_err(|e| {
                tracing::warn!(
                    file_name = %file_name,
                    mime_type = %mime_type,
                    error = %e,
                    "Failed to decode base64 text file"
                );
                e
            })
            .ok()?;

        let text_content = String::from_utf8(decoded)
            .map_err(|e| {
                tracing::warn!(
                    file_name = %file_name,
                    mime_type = %mime_type,
                    error = %e,
                    "Failed to decode text file as UTF-8"
                );
                e
            })
            .ok()?;

        let formatted = format!("[File: {file_name} ({mime_type})]\n{text_content}");
        Some(AiContentPart::text(formatted))
    }

    pub async fn process_message_stream(
        &self,
        a2a_message: &Message,
        agent_runtime: &AgentRuntimeInfo,
        agent_name: &str,
        context: &RequestContext,
        task_id: TaskId,
    ) -> Result<mpsc::UnboundedReceiver<StreamEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();

        let ai_service = self.ai_service.clone();
        let agent_runtime = agent_runtime.clone();
        let agent_name_string = agent_name.to_string();
        let agent_name_typed = AgentName::new(agent_name);
        let (user_text, user_parts) = Self::extract_message_content(a2a_message);

        let context_id = &a2a_message.context_id;
        let conversation_history = self
            .context_service
            .load_conversation_history(context_id.as_str())
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, context_id = %context_id, "Failed to load conversation history");
                vec![]
            });

        tracing::info!(
            context_id = %context_id,
            history_count = conversation_history.len(),
            "Loaded historical messages for context"
        );

        let context_id_str = context_id.to_string();
        let context_id_owned = context_id.clone();
        let task_id_str = task_id.to_string();

        let request_ctx = context
            .clone()
            .with_task_id(task_id.clone())
            .with_context_id(context_id.clone());
        let db_pool = self.db_pool.clone();
        let skill_service = self.skill_service.clone();
        let execution_step_repo = self.execution_step_repo.clone();

        tokio::spawn(async move {
            tracing::info!(
                agent_name = %agent_name_string,
                history_count = conversation_history.len(),
                "Processing streaming message for agent"
            );

            let mut ai_messages = Vec::new();

            if !agent_runtime.skills.is_empty() {
                tracing::info!(
                    skill_count = agent_runtime.skills.len(),
                    skills = ?agent_runtime.skills,
                    "Loading skills for agent"
                );

                let mut skills_prompt = String::from(
                    "# Your Skills\n\nYou have the following skills that define your capabilities \
                     and writing style:\n\n",
                );

                for skill_id in &agent_runtime.skills {
                    match skill_service.load_skill(skill_id, &request_ctx).await {
                        Ok(skill_content) => {
                            tracing::info!(
                                skill_id = %skill_id,
                                content_len = skill_content.len(),
                                "Loaded skill"
                            );
                            skills_prompt.push_str(&format!(
                                "## {} Skill\n\n{}\n\n---\n\n",
                                skill_id, skill_content
                            ));
                        },
                        Err(e) => {
                            tracing::warn!(skill_id = %skill_id, error = %e, "Failed to load skill");
                        },
                    }
                }

                ai_messages.push(AiMessage {
                    role: MessageRole::System,
                    content: skills_prompt,
                    parts: Vec::new(),
                });

                tracing::info!("Skills injected into agent context");
            }

            if let Some(system_prompt) = &agent_runtime.system_prompt {
                ai_messages.push(AiMessage {
                    role: MessageRole::System,
                    content: system_prompt.clone(),
                    parts: Vec::new(),
                });
            }

            ai_messages.extend(conversation_history);

            ai_messages.push(AiMessage {
                role: MessageRole::User,
                content: user_text,
                parts: user_parts,
            });

            let ai_messages_for_synthesis = ai_messages.clone();

            let has_tools = !agent_runtime.mcp_servers.is_empty();
            tracing::info!(
                mcp_server_count = agent_runtime.mcp_servers.len(),
                has_tools = has_tools,
                "Agent MCP server status"
            );

            let ai_service_for_builder = ai_service.clone();

            let selector = ExecutionStrategySelector::new();
            let strategy = selector.select_strategy(has_tools);

            let execution_context = ExecutionContext {
                ai_service: ai_service.clone(),
                skill_service: skill_service.clone(),
                agent_runtime: agent_runtime.clone(),
                agent_name: agent_name_typed.clone(),
                task_id: task_id.clone(),
                context_id: context_id_owned,
                tx: tx.clone(),
                request_ctx: request_ctx.clone(),
                execution_step_repo: execution_step_repo.clone(),
            };

            let execution_result = match strategy
                .execute(execution_context, ai_messages.clone())
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::error!(error = %e, "Execution failed");

                    let tracking =
                        crate::services::ExecutionTrackingService::new(execution_step_repo.clone());
                    if let Err(fail_err) = tracking
                        .fail_in_progress_steps(&task_id, &e.to_string())
                        .await
                    {
                        tracing::error!(error = %fail_err, "Failed to mark steps as failed");
                    }

                    tx.send(StreamEvent::Error(format!("Execution failed: {e}")))
                        .ok();
                    return;
                },
            };

            let (accumulated_text, tool_calls, tool_results, _iterations) = (
                execution_result.accumulated_text,
                execution_result.tool_calls,
                execution_result.tool_results,
                execution_result.iterations,
            );

            tracing::info!(
                text_len = accumulated_text.len(),
                tool_call_count = tool_calls.len(),
                tool_result_count = tool_results.len(),
                "Processing complete"
            );

            let artifacts = match build_artifacts_from_results(
                &tool_results,
                &tool_calls,
                db_pool.clone(),
                &context_id_str,
                &task_id_str,
            )
            .await
            {
                Ok(artifacts) => artifacts,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to build artifacts from tool results");
                    tx.send(StreamEvent::Error(format!("Artifact building failed: {e}")))
                        .ok();
                    return;
                },
            };

            let final_text = synthesize_final_response(
                &tool_calls,
                &tool_results,
                &artifacts,
                &accumulated_text,
                ai_service_for_builder,
                &agent_runtime,
                ai_messages_for_synthesis,
                tx.clone(),
                request_ctx.clone(),
                skill_service.clone(),
            )
            .await;

            tracing::info!(artifact_count = artifacts.len(), "Sending Complete event");

            for (idx, artifact) in artifacts.iter().enumerate() {
                tracing::info!(
                    artifact_index = idx + 1,
                    total_artifacts = artifacts.len(),
                    artifact_id = %artifact.id,
                    "Complete artifact"
                );
            }

            let send_result = tx.send(StreamEvent::Complete {
                full_text: final_text.clone(),
                artifacts: artifacts.clone(),
            });

            if send_result.is_err() {
                tracing::error!("Failed to send Complete event, channel closed");
            } else {
                tracing::info!(artifact_count = artifacts.len(), "Sent Complete event");
            }
        });

        Ok(rx)
    }
}

async fn build_artifacts_from_results(
    tool_results: &[systemprompt_models::CallToolResult],
    tool_calls: &[systemprompt_models::ToolCall],
    db_pool: DbPool,
    context_id_str: &str,
    task_id_str: &str,
) -> Result<Vec<Artifact>> {
    use crate::services::a2a_server::processing::artifact::DatabaseExecutionIdLookup;

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

    let execution_lookup = Arc::new(DatabaseExecutionIdLookup::new(db_pool));

    let artifact_builder = ArtifactBuilder::new(
        tool_calls.to_vec(),
        tool_results.to_vec(),
        execution_lookup,
        context_id_str.to_string(),
        task_id_str.to_string(),
    );

    artifact_builder.build_artifacts().await
}

async fn synthesize_final_response(
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
