use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::helpers::{build_artifacts_from_results, synthesize_final_response};
use super::StreamProcessor;
use crate::models::a2a::Message;
use crate::models::AgentRuntimeInfo;
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::a2a_server::processing::strategies::{
    ExecutionContext, ExecutionStrategySelector,
};
use systemprompt_identifiers::{AgentName, TaskId};
use systemprompt_models::{AiMessage, MessageRole, RequestContext};

impl StreamProcessor {
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
        let skill_service = self.skill_service.clone();
        let execution_step_repo = self.execution_step_repo.clone();

        tokio::spawn(async move {
            tracing::info!(
                agent_name = %agent_name_string,
                history_count = conversation_history.len(),
                "Processing streaming message for agent"
            );

            let ai_messages = build_ai_messages(
                &agent_runtime,
                conversation_history,
                user_text,
                user_parts,
                &skill_service,
                &request_ctx,
            )
            .await;

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

            let (accumulated_text, tool_calls, tool_results, tools, _iterations) = (
                execution_result.accumulated_text,
                execution_result.tool_calls,
                execution_result.tool_results,
                execution_result.tools,
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
                &tools,
                &context_id_str,
                &task_id_str,
            ) {
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

async fn build_ai_messages(
    agent_runtime: &AgentRuntimeInfo,
    conversation_history: Vec<AiMessage>,
    user_text: String,
    user_parts: Vec<systemprompt_models::AiContentPart>,
    skill_service: &Arc<crate::services::SkillService>,
    request_ctx: &RequestContext,
) -> Vec<AiMessage> {
    let mut ai_messages = Vec::new();

    if !agent_runtime.skills.is_empty() {
        tracing::info!(
            skill_count = agent_runtime.skills.len(),
            skills = ?agent_runtime.skills,
            "Loading skills for agent"
        );

        let mut skills_prompt = String::from(
            "# Your Skills\n\nYou have the following skills that define your capabilities and \
             writing style:\n\n",
        );

        for skill_id in &agent_runtime.skills {
            match skill_service.load_skill(skill_id, request_ctx).await {
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

    ai_messages
}
