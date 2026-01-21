mod direct_response;
mod helpers;
mod tool_execution;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_identifiers::TaskId;
use systemprompt_models::ai::PlanningResult;
use systemprompt_models::AiMessage;

use super::{ExecutionContext, ExecutionResult, ExecutionStrategy};
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::ExecutionTrackingService;
use helpers::build_ai_request;

#[derive(Debug, Clone, Copy)]
pub struct PlannedAgenticStrategy;

impl PlannedAgenticStrategy {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PlannedAgenticStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionStrategy for PlannedAgenticStrategy {
    async fn execute(
        &self,
        context: ExecutionContext,
        messages: Vec<AiMessage>,
    ) -> Result<ExecutionResult> {
        let tracking = ExecutionTrackingService::new(context.execution_step_repo.clone());
        let task_id = TaskId::new(context.task_id.as_str());

        tracing::info!("Starting PLAN → EXECUTE → RESPOND flow");

        if let Ok(step) = tracking.track_understanding(task_id.clone()).await {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped");
            }
        }

        let tools = context
            .ai_service
            .list_available_tools_for_agent(&context.agent_name, &context.request_ctx)
            .await?;

        tracing::info!(tool_count = tools.len(), "Available tools");

        let planning_tracked = tracking
            .track_planning_async(task_id.clone(), None, None)
            .await;

        if let Ok((_, ref step)) = planning_tracked {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step: step.clone() })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped");
            }
        }

        let request = build_ai_request(&context, messages.clone());

        let planning_result = context.ai_service.generate_plan(&request, &tools).await;

        let planning_result = match planning_result {
            Ok(result) => result,
            Err(e) => {
                if let Ok((tracked, _)) = planning_tracked {
                    if let Err(fail_err) = tracking.fail(&tracked, e.to_string()).await {
                        tracing::warn!(error = %fail_err, "Failed to record planning failure");
                    }
                }
                return Err(e);
            },
        };

        match planning_result {
            PlanningResult::DirectResponse { content } => {
                direct_response::handle_direct_response(
                    content,
                    &context,
                    &tracking,
                    planning_tracked,
                    task_id,
                )
                .await
            },

            PlanningResult::ToolCalls { reasoning, calls } => {
                tool_execution::handle_tool_calls(
                    reasoning,
                    calls,
                    &context,
                    &tracking,
                    planning_tracked,
                    task_id,
                    messages,
                    tools,
                )
                .await
            },
        }
    }

    fn name(&self) -> &'static str {
        "planned"
    }
}
