//! The spawned streaming pipeline run.
//!
//! Implements [`StreamProcessor::process_message_stream`] and the worker it
//! spawns: it assembles AI messages, selects an execution strategy, runs it,
//! builds artifacts, synthesizes a final response, and emits exactly one
//! terminal event (`Complete`, `Error`, or `Cancelled`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod execution;
mod messages;

use std::sync::Arc;

use tokio::sync::mpsc;

use self::execution::{build_artifacts, run_strategy};
use self::messages::{BuildAiMessagesParams, build_ai_messages};
use super::StreamProcessor;
use super::helpers::{SynthesizeFinalResponseParams, synthesize_final_response};
use crate::models::AgentRuntimeInfo;
use crate::models::a2a::Artifact;
use crate::services::a2a_server::processing::message::content::extract_message_content;
use crate::services::a2a_server::processing::message::{
    MessageStream, ProcessMessageStreamParams, StreamEvent,
};
use crate::services::a2a_server::processing::strategies::ExecutionContext;
use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::AgentName;
use systemprompt_models::{AiMessage, RequestContext};

impl StreamProcessor {
    pub async fn process_message_stream(
        &self,
        params: ProcessMessageStreamParams<'_>,
    ) -> Result<MessageStream> {
        let ProcessMessageStreamParams {
            a2a_message,
            agent_runtime,
            agent_name,
            context,
            task_id,
            cancel,
        } = params;
        let (tx, rx) = mpsc::channel(1024);

        let ai_service = Arc::clone(&self.ai_service);
        let agent_runtime = agent_runtime.clone();
        let agent_name_string = agent_name.to_owned();
        let agent_name_typed = AgentName::try_new(agent_name)
            .map_err(|e| AgentServiceError::Validation("agent_name".to_owned(), e.to_string()))?;
        let (user_text, user_parts) = extract_message_content(a2a_message);

        let context_id = &a2a_message.context_id;
        let conversation_history = self
            .context_service
            .load_conversation_history(context_id)
            .await?;

        tracing::info!(
            context_id = %context_id,
            history_count = conversation_history.len(),
            "Loaded historical messages for context"
        );

        let request_ctx = context
            .clone()
            .with_task_id(task_id.clone())
            .with_context_id(context_id.clone());
        let pipeline = RunStreamPipelineParams {
            agent_runtime,
            agent_name_string,
            agent_name_typed,
            ai_service,
            skill_service: Arc::clone(&self.skill_service),
            execution_step_repo: Arc::clone(&self.execution_step_repo),
            task_id,
            context_id: context_id.clone(),
            request_ctx,
            conversation_history,
            user_text,
            user_parts,
            tx: tx.clone(),
        };

        let worker_cancel = cancel.clone();
        let worker = tokio::spawn(async move {
            tokio::select! {
                () = worker_cancel.cancelled() => {
                    if tx.send(StreamEvent::Cancelled).await.is_err() {
                        tracing::debug!("Stream receiver dropped before cancellation was reported");
                    }
                },
                () = run_stream_pipeline(pipeline) => {},
            }
        });

        Ok(MessageStream {
            events: rx,
            worker,
            cancel,
        })
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
    context_id: systemprompt_identifiers::ContextId,
    request_ctx: RequestContext,
    conversation_history: Vec<AiMessage>,
    user_text: String,
    user_parts: Vec<systemprompt_models::AiContentPart>,
    tx: mpsc::Sender<StreamEvent>,
}

async fn run_stream_pipeline(params: RunStreamPipelineParams) {
    let tx = params.tx.clone();
    match run_pipeline(params).await {
        Ok((final_text, artifacts)) => send_complete_event(&tx, final_text, artifacts).await,
        Err(AgentServiceError::StreamClosed) => {
            tracing::debug!("Stream receiver dropped; pipeline stopped");
        },
        Err(e) => report_stream_error(&tx, e.to_string()).await,
    }
}

async fn run_pipeline(params: RunStreamPipelineParams) -> Result<(String, Vec<Artifact>)> {
    let RunStreamPipelineParams {
        agent_runtime,
        agent_name_string,
        agent_name_typed,
        ai_service,
        skill_service,
        execution_step_repo,
        task_id,
        context_id,
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
    .await?;

    let ai_messages_for_synthesis = ai_messages.clone();
    let ai_service_for_builder = Arc::clone(&ai_service);

    let execution_context = ExecutionContext {
        ai_service: Arc::clone(&ai_service),
        skill_service: Arc::clone(&skill_service),
        agent_runtime: agent_runtime.clone(),
        agent_name: agent_name_typed,
        task_id: task_id.clone(),
        context_id: context_id.clone(),
        tx: tx.clone(),
        request_ctx: request_ctx.clone(),
        execution_step_repo: Arc::clone(&execution_step_repo),
    };

    let execution_result = run_strategy(execution_context, ai_messages).await?;
    let artifacts = build_artifacts(&execution_result, &context_id, &task_id)?;

    let final_text = synthesize_final_response(SynthesizeFinalResponseParams {
        tool_calls: &execution_result.tool_calls,
        tool_results: &execution_result.tool_results,
        artifacts: &artifacts,
        accumulated_text: &execution_result.accumulated_text,
        ai_service: ai_service_for_builder,
        agent_runtime: &agent_runtime,
        ai_messages_for_synthesis,
        tx: tx.clone(),
        request_ctx,
        skill_service: Arc::clone(&skill_service),
    })
    .await?;

    Ok((final_text, artifacts))
}

async fn report_stream_error(tx: &mpsc::Sender<StreamEvent>, message: String) {
    if tx.send(StreamEvent::Error(message)).await.is_err() {
        tracing::trace!("Failed to send error event, channel closed");
    }
}

async fn send_complete_event(
    tx: &mpsc::Sender<StreamEvent>,
    final_text: String,
    artifacts: Vec<Artifact>,
) {
    let artifact_count = artifacts.len();
    tracing::info!(artifact_count, "Sending Complete event");

    if tx
        .send(StreamEvent::Complete {
            full_text: final_text,
            artifacts,
        })
        .await
        .is_err()
    {
        tracing::error!("Failed to send Complete event, channel closed");
    }
}
