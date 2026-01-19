use anyhow::Result;
use async_trait::async_trait;
use systemprompt_identifiers::TaskId;
use systemprompt_models::AiMessage;

use super::{ExecutionContext, ExecutionResult, ExecutionStrategy};
use crate::services::a2a_server::processing::ai_executor::process_without_tools;
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::ExecutionTrackingService;

#[derive(Debug, Clone, Copy)]
pub struct StandardExecutionStrategy;

impl StandardExecutionStrategy {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for StandardExecutionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionStrategy for StandardExecutionStrategy {
    async fn execute(
        &self,
        context: ExecutionContext,
        messages: Vec<AiMessage>,
    ) -> Result<ExecutionResult> {
        tracing::info!("Processing without tools");

        let tracking = ExecutionTrackingService::new(context.execution_step_repo.clone());
        let task_id = TaskId::new(context.task_id.as_str());

        if let Ok(step) = tracking.track_understanding(task_id.clone()).await {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped during execution");
            }
        }

        let (accumulated_text, tool_calls, tool_results) = process_without_tools(
            context.ai_service.clone(),
            &context.agent_runtime,
            messages,
            context.tx.clone(),
            context.request_ctx.clone(),
        )
        .await
        .map_err(|()| anyhow::anyhow!("Standard execution failed - see stream errors for details"))?;

        if let Ok(step) = tracking.track_completion(task_id).await {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped during completion");
            }
        }

        Ok(ExecutionResult {
            accumulated_text,
            tool_calls,
            tool_results,
            tools: vec![],
            iterations: 1,
        })
    }

    fn name(&self) -> &'static str {
        "standard"
    }
}
