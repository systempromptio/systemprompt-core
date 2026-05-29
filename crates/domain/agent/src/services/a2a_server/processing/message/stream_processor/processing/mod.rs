//! The spawned streaming pipeline run.
//!
//! Implements [`StreamProcessor::process_message_stream`] and the background
//! task it spawns: it assembles AI messages, selects an execution strategy,
//! runs it, builds artifacts, synthesizes a final response, and emits a
//! `Complete` event.

mod messages;

use std::sync::Arc;

use tokio::sync::mpsc;

use self::messages::{BuildAiMessagesParams, build_ai_messages};
use super::StreamProcessor;
use super::helpers::{
    SynthesizeFinalResponseParams, build_artifacts_from_results, synthesize_final_response,
};
use crate::models::AgentRuntimeInfo;
use crate::services::a2a_server::processing::message::{ProcessMessageStreamParams, StreamEvent};
use crate::services::a2a_server::processing::strategies::{
    ExecutionContext, ExecutionStrategySelector,
};
use crate::services::shared::Result;
use systemprompt_identifiers::AgentName;
use systemprompt_models::{AiMessage, RequestContext};

impl StreamProcessor {
    pub async fn process_message_stream(
        &self,
        params: ProcessMessageStreamParams<'_>,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let ProcessMessageStreamParams {
            a2a_message,
            agent_runtime,
            agent_name,
            context,
            task_id,
        } = params;
        let (tx, rx) = mpsc::channel(1024);

        let ai_service = Arc::clone(&self.ai_service);
        let agent_runtime = agent_runtime.clone();
        let agent_name_string = agent_name.to_owned();
        let agent_name_typed = AgentName::new(agent_name);
        let (user_text, user_parts) = Self::extract_message_content(a2a_message);

        let context_id = &a2a_message.context_id;
        let conversation_history = self
            .context_service
            .load_conversation_history(context_id)
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

        let context_id_for_artifacts = context_id.clone();
        let context_id_owned = context_id.clone();
        let task_id_for_artifacts = task_id.clone();

        let request_ctx = context
            .clone()
            .with_task_id(task_id.clone())
            .with_context_id(context_id.clone());
        let skill_service = Arc::clone(&self.skill_service);
        let execution_step_repo = Arc::clone(&self.execution_step_repo);

        tokio::spawn(run_stream_pipeline(RunStreamPipelineParams {
            agent_runtime,
            agent_name_string,
            agent_name_typed,
            ai_service,
            skill_service,
            execution_step_repo,
            task_id,
            context_id_owned,
            context_id_for_artifacts,
            task_id_for_artifacts,
            request_ctx,
            conversation_history,
            user_text,
            user_parts,
            tx,
        }));

        Ok(rx)
    }
}

struct RunStreamPipelineParams {
    agent_runtime: AgentRuntimeInfo,
    agent_name_string: String,
    agent_name_typed: AgentName,
    ai_service: Arc<dyn systemprompt_models::AiProvider>,
    skill_service: Arc<crate::services::SkillService>,
    execution_step_repo: Arc<crate::repository::execution::ExecutionStepRepository>,
    task_id: systemprompt_identifiers::TaskId,
    context_id_owned: systemprompt_identifiers::ContextId,
    context_id_for_artifacts: systemprompt_identifiers::ContextId,
    task_id_for_artifacts: systemprompt_identifiers::TaskId,
    request_ctx: RequestContext,
    conversation_history: Vec<AiMessage>,
    user_text: String,
    user_parts: Vec<systemprompt_models::AiContentPart>,
    tx: mpsc::Sender<StreamEvent>,
}

async fn run_stream_pipeline(params: RunStreamPipelineParams) {
    let RunStreamPipelineParams {
        agent_runtime,
        agent_name_string,
        agent_name_typed,
        ai_service,
        skill_service,
        execution_step_repo,
        task_id,
        context_id_owned,
        context_id_for_artifacts,
        task_id_for_artifacts,
        request_ctx,
        conversation_history,
        user_text,
        user_parts,
        tx,
    } = params;

    tracing::info!(
        agent_name = %agent_name_string,
        history_count = conversation_history.len(),
        "Processing streaming message for agent"
    );

    let ai_messages = build_ai_messages(BuildAiMessagesParams {
        agent_runtime: &agent_runtime,
        conversation_history,
        user_text,
        user_parts,
        skill_service: &skill_service,
        request_ctx: &request_ctx,
    })
    .await;

    let ai_messages_for_synthesis = ai_messages.clone();

    let has_tools = !agent_runtime.mcp_servers.include.is_empty();
    tracing::info!(
        mcp_server_count = agent_runtime.mcp_servers.include.len(),
        has_tools = has_tools,
        "Agent MCP server status"
    );

    let ai_service_for_builder = Arc::clone(&ai_service);
    let strategy = ExecutionStrategySelector::select_strategy(has_tools);

    let execution_context = ExecutionContext {
        ai_service: Arc::clone(&ai_service),
        skill_service: Arc::clone(&skill_service),
        agent_runtime: agent_runtime.clone(),
        agent_name: agent_name_typed,
        task_id: task_id.clone(),
        context_id: context_id_owned,
        tx: tx.clone(),
        request_ctx: request_ctx.clone(),
        execution_step_repo: Arc::clone(&execution_step_repo),
    };

    let execution_result = match strategy.execute(execution_context, ai_messages).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!(error = %e, "Execution failed");
            let tracking =
                crate::services::ExecutionTrackingService::new(Arc::clone(&execution_step_repo));
            if let Err(fail_err) = tracking
                .fail_in_progress_steps(&task_id, &e.to_string())
                .await
            {
                tracing::error!(error = %fail_err, "Failed to mark steps as failed");
            }
            if let Err(send_err) = tx.try_send(StreamEvent::Error(format!("Execution failed: {e}")))
            {
                tracing::trace!(error = %send_err, "Failed to send error event, channel closed");
            }
            return;
        },
    };

    let accumulated_text = execution_result.accumulated_text;
    let tool_calls = execution_result.tool_calls;
    let tool_results = execution_result.tool_results;
    let tools = execution_result.tools;

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
        &context_id_for_artifacts,
        &task_id_for_artifacts,
    ) {
        Ok(artifacts) => artifacts,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build artifacts from tool results");
            if let Err(send_err) =
                tx.try_send(StreamEvent::Error(format!("Artifact building failed: {e}")))
            {
                tracing::trace!(error = %send_err, "Failed to send error event, channel closed");
            }
            return;
        },
    };

    let final_text = synthesize_final_response(SynthesizeFinalResponseParams {
        tool_calls: &tool_calls,
        tool_results: &tool_results,
        artifacts: &artifacts,
        accumulated_text: &accumulated_text,
        ai_service: ai_service_for_builder,
        agent_runtime: &agent_runtime,
        ai_messages_for_synthesis,
        tx: tx.clone(),
        request_ctx,
        skill_service: Arc::clone(&skill_service),
    })
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

    let send_result = tx.try_send(StreamEvent::Complete {
        full_text: final_text,
        artifacts: artifacts.clone(),
    });
    if send_result.is_err() {
        tracing::error!("Failed to send Complete event, channel closed");
    } else {
        tracing::info!(artifact_count = artifacts.len(), "Sent Complete event");
    }
}
