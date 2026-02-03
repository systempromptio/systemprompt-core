use anyhow::Result;
use uuid::Uuid;

use crate::models::ai::{AiMessage, AiRequest, AiResponse, GenerateResponseParams};
use crate::models::tools::McpTool;
use crate::models::RequestStatus;
use crate::services::providers::{GenerationParams, ModelPricing, ToolGenerationParams};

use super::super::request_storage::StoreParams;
use super::service::AiService;

impl AiService {
    pub async fn generate_plan(
        &self,
        request: &AiRequest,
        available_tools: &[McpTool],
    ) -> Result<systemprompt_models::ai::PlanningResult> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
        let provider = self.get_provider(request.provider())?;
        let model = request.model();

        let base = GenerationParams::new(&request.messages, model, request.max_output_tokens());
        let base = request
            .sampling
            .as_ref()
            .map_or_else(|| base.clone(), |s| base.clone().with_sampling(s));
        let params = ToolGenerationParams::new(base, available_tools.to_vec());

        let result = provider.generate_with_tools(params).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((mut response, tool_calls)) => {
                response.request_id = request_id;
                response.latency_ms = latency_ms;
                response.tool_calls.clone_from(&tool_calls);
                let cost = self.estimate_cost(&response);
                self.storage.store(&StoreParams {
                    request,
                    response: &response,
                    context: &request.context,
                    status: RequestStatus::Completed,
                    error_message: None,
                    cost_microdollars: cost,
                });

                Ok(if tool_calls.is_empty() {
                    systemprompt_models::ai::PlanningResult::DirectResponse {
                        content: response.content,
                    }
                } else {
                    systemprompt_models::ai::PlanningResult::ToolCalls {
                        reasoning: response.content,
                        calls: tool_calls
                            .into_iter()
                            .map(|tc| systemprompt_models::ai::PlannedToolCall {
                                tool_name: tc.name,
                                arguments: tc.arguments,
                            })
                            .collect(),
                    }
                })
            },
            Err(e) => {
                self.store_error(request, request_id, latency_ms, &e);
                Err(e)
            },
        }
    }

    pub async fn generate_response(&self, params: GenerateResponseParams<'_>) -> Result<String> {
        let mut response_messages = params.messages;
        response_messages.push(AiMessage::user(format!(
            "## Tool Execution Complete\n\nThe following tools have been executed:\n\n{}\n\n## \
             Response Phase Instructions\n\nThis is the RESPONSE PHASE - your task is to \
             synthesize results and respond to the user.\n\n**CRITICAL: Do NOT attempt to call \
             any tools.** Tools are not available in this phase.\nAnalyze the tool results and \
             provide a helpful response to the user.",
            params.execution_summary
        )));

        let tool_config = params.context.tool_model_config();

        let provider = tool_config
            .and_then(|c| c.provider.as_deref())
            .or(params.provider)
            .unwrap_or_else(|| self.default_provider());
        let model = tool_config
            .and_then(|c| c.model.as_deref())
            .or(params.model)
            .unwrap_or_else(|| self.default_model());
        let max_output_tokens = tool_config
            .and_then(|c| c.max_output_tokens)
            .or(params.max_output_tokens)
            .unwrap_or_else(|| self.default_max_output_tokens());

        if tool_config.is_some() {
            tracing::debug!(
                provider,
                model,
                max_output_tokens,
                "Using tool_model_config in generate_response"
            );
        }

        let request = AiRequest::builder(
            response_messages,
            provider,
            model,
            max_output_tokens,
            params.context.clone(),
        )
        .build();

        let response = self.generate(&request).await?;
        Ok(response.content)
    }

    pub(super) fn estimate_cost(&self, response: &AiResponse) -> i64 {
        let input = f64::from(response.input_tokens.unwrap_or(0));
        let output = f64::from(response.output_tokens.unwrap_or(0));

        let pricing = self
            .providers
            .get(&response.provider)
            .map_or(ModelPricing::new(0.001, 0.001), |p| {
                p.get_pricing(&response.model)
            });

        let input_cost = (input / 1000.0) * f64::from(pricing.input_cost_per_1k);
        let output_cost = (output / 1000.0) * f64::from(pricing.output_cost_per_1k);

        ((input_cost + output_cost) * 1_000_000.0).round() as i64
    }
}
