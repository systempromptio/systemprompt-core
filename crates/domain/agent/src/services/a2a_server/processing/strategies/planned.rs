use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_identifiers::TaskId;
use systemprompt_models::ai::{GenerateResponseParams, PlanningResult, TemplateValidator};
use systemprompt_models::{AiMessage, AiRequest, PlannedTool};

use super::plan_executor::{
    convert_to_call_tool_results, convert_to_tool_calls, execute_tools_with_templates,
    format_results_for_response,
};
use super::tool_executor::ContextToolExecutor;
use super::{ExecutionContext, ExecutionResult, ExecutionStrategy};
use crate::services::a2a_server::processing::message::StreamEvent;
use crate::services::ExecutionTrackingService;

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

fn build_ai_request(context: &ExecutionContext, messages: Vec<AiMessage>) -> AiRequest {
    let provider = context
        .agent_runtime
        .provider
        .as_deref()
        .unwrap_or_else(|| context.ai_service.default_provider());
    let model = context
        .agent_runtime
        .model
        .as_deref()
        .unwrap_or_else(|| context.ai_service.default_model());

    AiRequest::builder(
        messages,
        provider,
        model,
        context.ai_service.default_max_output_tokens(),
        context.request_ctx.clone(),
    )
    .build()
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
                    iterations: 1,
                })
            },

            PlanningResult::ToolCalls { reasoning, calls } => {
                tracing::info!(
                    tool_count = calls.len(),
                    reasoning = %reasoning,
                    "Tool calls planned"
                );

                let planned_tools: Vec<PlannedTool> = calls
                    .iter()
                    .map(|c| PlannedTool {
                        tool_name: c.tool_name.clone(),
                        arguments: c.arguments.clone(),
                    })
                    .collect();

                if let Ok((tracked, _)) = planning_tracked {
                    if let Ok(step) = tracking
                        .complete_planning(tracked, Some(reasoning.clone()), Some(planned_tools))
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

                let tool_output_schemas =
                    TemplateValidator::get_tool_output_schemas(&calls, &tools);

                if let Err(validation_errors) =
                    TemplateValidator::validate_plan(&calls, &tool_output_schemas)
                {
                    let error_messages: Vec<String> =
                        validation_errors.iter().map(|e| e.to_string()).collect();

                    tracing::error!(
                        errors = ?error_messages,
                        "Template validation failed"
                    );

                    let validation_summary = format!(
                        "Plan validation failed:\n{}",
                        error_messages
                            .iter()
                            .map(|e| format!("- {e}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );

                    let response = context
                        .ai_service
                        .generate_response(GenerateResponseParams {
                            messages,
                            execution_summary: &validation_summary,
                            context: &context.request_ctx,
                            provider: context.agent_runtime.provider.as_deref(),
                            model: context.agent_runtime.model.as_deref(),
                        })
                        .await?;

                    if context
                        .tx
                        .send(StreamEvent::Text(response.clone()))
                        .is_err()
                    {
                        tracing::debug!("Stream receiver dropped");
                    }

                    return Ok(ExecutionResult {
                        accumulated_text: response,
                        tool_calls: vec![],
                        tool_results: vec![],
                        iterations: 1,
                    });
                }

                tracing::info!("Template validation passed");

                let (tool_name, tool_arguments) = if calls.len() == 1 {
                    (calls[0].tool_name.clone(), calls[0].arguments.clone())
                } else {
                    let tool_args_summary: Vec<Value> = calls
                        .iter()
                        .map(|c| {
                            serde_json::json!({
                                "tool": c.tool_name,
                                "arguments": c.arguments
                            })
                        })
                        .collect();
                    (
                        format!("{} tools", calls.len()),
                        serde_json::json!(tool_args_summary),
                    )
                };

                let (tracked, step) = tracking
                    .track_tool_execution(task_id.clone(), tool_name, tool_arguments)
                    .await?;

                if context
                    .tx
                    .send(StreamEvent::ExecutionStepUpdate { step })
                    .is_err()
                {
                    tracing::debug!("Stream receiver dropped");
                }

                let tool_executor = ContextToolExecutor {
                    context: context.clone(),
                };

                let state = execute_tools_with_templates(
                    &calls,
                    &tools,
                    &context.request_ctx,
                    &tool_executor,
                )
                .await?;

                let execution_summary = format_results_for_response(&state);

                let has_failures = !state.failed_results().is_empty();

                if has_failures {
                    let error_message = state
                        .failed_results()
                        .iter()
                        .filter_map(|r| r.error.as_ref())
                        .map(|e| e.as_str())
                        .collect::<Vec<_>>()
                        .join("; ");

                    if let Err(e) = tracking.fail(&tracked, error_message).await {
                        tracing::warn!(error = %e, "Failed to record execution failure");
                    }
                } else {
                    let tool_result = if state.results.len() == 1 {
                        serde_json::json!({
                            "tool": state.results[0].tool_name,
                            "output": state.results[0].output,
                            "duration_ms": state.results[0].duration_ms
                        })
                    } else {
                        serde_json::json!({
                            "results": state.results.iter().map(|r| {
                                serde_json::json!({
                                    "tool": r.tool_name,
                                    "output": r.output,
                                    "duration_ms": r.duration_ms
                                })
                            }).collect::<Vec<_>>()
                        })
                    };

                    if let Err(e) = tracking.complete(tracked, Some(tool_result)).await {
                        tracing::warn!(error = %e, "Failed to record execution completion");
                    }
                }

                tracing::info!(
                    succeeded = state.successful_results().len(),
                    failed = state.failed_results().len(),
                    "Execution complete"
                );

                if let Ok(step) = tracking.track_completion(task_id).await {
                    if context
                        .tx
                        .send(StreamEvent::ExecutionStepUpdate { step })
                        .is_err()
                    {
                        tracing::debug!("Stream receiver dropped");
                    }
                }

                let response = context
                    .ai_service
                    .generate_response(GenerateResponseParams {
                        messages,
                        execution_summary: &execution_summary,
                        context: &context.request_ctx,
                        provider: context.agent_runtime.provider.as_deref(),
                        model: context.agent_runtime.model.as_deref(),
                    })
                    .await?;

                if context
                    .tx
                    .send(StreamEvent::Text(response.clone()))
                    .is_err()
                {
                    tracing::debug!("Stream receiver dropped");
                }

                let tool_calls = convert_to_tool_calls(&calls);
                let tool_results = convert_to_call_tool_results(&state);

                Ok(ExecutionResult {
                    accumulated_text: response,
                    tool_calls,
                    tool_results,
                    iterations: 1,
                })
            },
        }
    }

    fn name(&self) -> &'static str {
        "planned"
    }
}
