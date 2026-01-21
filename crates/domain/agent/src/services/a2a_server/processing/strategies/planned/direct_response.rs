use anyhow::Result;
use systemprompt_identifiers::TaskId;
use systemprompt_models::{ExecutionStep, TrackedStep};

use super::super::{ExecutionContext, ExecutionResult};
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::ExecutionTrackingService;

pub async fn handle_direct_response(
    content: String,
    context: &ExecutionContext,
    tracking: &ExecutionTrackingService,
    planning_tracked: Result<(TrackedStep, ExecutionStep), anyhow::Error>,
    task_id: TaskId,
) -> Result<ExecutionResult> {
    if let Ok((tracked, _)) = planning_tracked {
        if let Ok(step) = tracking
            .complete_planning(
                tracked,
                Some("Direct response - no tools needed".to_string()),
                None,
            )
            .await
        {
            if context
                .tx
                .send(StreamEvent::ExecutionStepUpdate { step })
                .is_err()
            {
                tracing::debug!("Stream receiver dropped");
            }
        }
    }

    tracing::info!("Direct response (no tools needed)");

    if let Ok(step) = tracking.track_completion(task_id).await {
        if context
            .tx
            .send(StreamEvent::ExecutionStepUpdate { step })
            .is_err()
        {
            tracing::debug!("Stream receiver dropped");
        }
    }

    if context.tx.send(StreamEvent::Text(content.clone())).is_err() {
        tracing::debug!("Stream receiver dropped");
    }

    Ok(ExecutionResult {
        accumulated_text: content,
        tool_calls: vec![],
        tool_results: vec![],
        tools: vec![],
        iterations: 1,
    })
}
