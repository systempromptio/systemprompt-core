//! Direct-response path of the planned execution strategy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::Result;
use systemprompt_identifiers::TaskId;
use systemprompt_models::TrackedStep;

use super::super::{ExecutionContext, ExecutionResult};
use crate::services::ExecutionTrackingService;
use crate::services::a2a_server::processing::message::StreamEvent;

pub(super) async fn handle_direct_response(
    response_text: String,
    exec_ctx: &ExecutionContext,
    tracking: &ExecutionTrackingService,
    planning_tracked: TrackedStep,
    task_id: TaskId,
) -> Result<ExecutionResult> {
    let step = tracking
        .complete_planning(
            planning_tracked,
            Some("Direct response - no tools needed".to_owned()),
            None,
        )
        .await?;
    exec_ctx
        .emit(StreamEvent::ExecutionStepUpdate { step })
        .await?;

    tracing::info!("Direct response (no tools needed)");

    let step = tracking.track_completion(task_id).await?;
    exec_ctx
        .emit(StreamEvent::ExecutionStepUpdate { step })
        .await?;
    exec_ctx
        .emit(StreamEvent::Text(response_text.clone()))
        .await?;

    Ok(ExecutionResult {
        accumulated_text: response_text,
        tool_calls: vec![],
        tool_results: vec![],
        tools: vec![],
        iterations: 1,
    })
}
