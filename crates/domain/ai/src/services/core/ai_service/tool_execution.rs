use anyhow::Result;
use uuid::Uuid;

use crate::models::ai::{AiRequest, AiResponse};
use crate::models::tools::{CallToolResult, McpTool, ToolCall};
use crate::models::RequestStatus;
use crate::services::providers::{AiProvider, GenerationParams, ToolGenerationParams};
use crate::services::tooled::{ResponseStrategy, SynthesisParams};

use super::super::request_logging;
use super::super::request_storage::StoreParams;
use super::service::AiService;

use systemprompt_models::RequestContext;

struct FinalizeTooledParams<'a> {
    ai_result: Result<(AiResponse, Vec<ToolCall>)>,
    request_id: Uuid,
    latency_ms: u64,
    request: &'a AiRequest,
    model: &'a str,
    provider: &'a dyn AiProvider,
    tools: &'a [McpTool],
}

struct SynthesizeIfNeededParams<'a> {
    response: &'a AiResponse,
    tool_calls: &'a [ToolCall],
    tool_results: &'a [CallToolResult],
    provider: &'a dyn AiProvider,
    request: &'a AiRequest,
    model: &'a str,
}

impl AiService {
    pub async fn generate_with_tools(&self, request: &AiRequest) -> Result<AiResponse> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
        let provider = self.get_provider(request.provider())?;
        let model = request.model();
        let tools = request.tools.as_deref().unwrap_or(&[]);

        request_logging::log_tooled_request_start(request_id, request, request.provider(), model);

        let base = GenerationParams::new(&request.messages, model, request.max_output_tokens());
        let base = request
            .sampling
            .as_ref()
            .map_or_else(|| base.clone(), |s| base.clone().with_sampling(s));
        let params = ToolGenerationParams::new(base, tools.to_vec());

        let ai_result = provider.generate_with_tools(params).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        self.finalize_tooled_response(FinalizeTooledParams {
            ai_result,
            request_id,
            latency_ms,
            request,
            model,
            provider: provider.as_ref(),
            tools,
        })
        .await
    }

    async fn finalize_tooled_response(
        &self,
        params: FinalizeTooledParams<'_>,
    ) -> Result<AiResponse> {
        let FinalizeTooledParams {
            ai_result,
            request_id,
            latency_ms,
            request,
            model,
            provider,
            tools,
        } = params;

        let (response, tool_calls) = match ai_result {
            Ok(result) => result,
            Err(e) => {
                self.store_error(request, request_id, latency_ms, &e);
                return Err(e);
            },
        };

        request_logging::log_ai_response(&response, tool_calls.len());

        let (tool_calls, tool_results) = self
            .tooled_executor
            .execute_tool_calls(tool_calls, tools, &request.context, None)
            .await;

        let final_content = self
            .synthesize_if_needed(SynthesizeIfNeededParams {
                response: &response,
                tool_calls: &tool_calls,
                tool_results: &tool_results,
                provider,
                request,
                model,
            })
            .await;

        let cost = self.estimate_cost(&response);
        let mut storage_response = response.clone();
        storage_response.request_id = request_id;
        storage_response.latency_ms = latency_ms;
        storage_response.tool_calls.clone_from(&tool_calls);
        storage_response.tool_results.clone_from(&tool_results);
        self.storage.store(&StoreParams {
            request,
            response: &storage_response,
            context: &request.context,
            status: RequestStatus::Completed,
            error_message: None,
            cost_cents: cost,
        });

        let final_response = AiResponse::new(
            request_id,
            final_content,
            request.provider().to_string(),
            model.to_string(),
        )
        .with_latency(latency_ms)
        .with_tool_calls(tool_calls)
        .with_tool_results(tool_results);

        request_logging::log_tooled_response(&final_response);
        Ok(final_response)
    }

    async fn synthesize_if_needed(&self, params: SynthesizeIfNeededParams<'_>) -> String {
        let SynthesizeIfNeededParams {
            response,
            tool_calls,
            tool_results,
            provider,
            request,
            model,
        } = params;

        let strategy = ResponseStrategy::from_response(
            response.content.clone(),
            tool_calls.to_vec(),
            tool_results.to_vec(),
        );

        match strategy {
            ResponseStrategy::ContentProvided { content, .. } => content,
            ResponseStrategy::ArtifactsProvided { .. } => String::new(),
            ResponseStrategy::ToolsOnly {
                tool_calls,
                tool_results,
            } => {
                self.synthesizer
                    .synthesize_or_fallback(SynthesisParams {
                        provider,
                        original_messages: &[],
                        tool_calls: &tool_calls,
                        tool_results: &tool_results,
                        sampling: request.sampling.as_ref(),
                        max_output_tokens: request.max_output_tokens(),
                        model,
                    })
                    .await
            },
        }
    }

    pub async fn generate_single_turn(
        &self,
        request: &AiRequest,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
        let provider = self.get_provider(request.provider())?;
        let model = request.model();
        let tools = request.tools.clone().unwrap_or_default();

        let base = GenerationParams::new(&request.messages, model, request.max_output_tokens());
        let base = request
            .sampling
            .as_ref()
            .map_or_else(|| base.clone(), |s| base.clone().with_sampling(s));
        let params = ToolGenerationParams::new(base, tools);

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
                    cost_cents: cost,
                });
                Ok((response, tool_calls))
            },
            Err(e) => {
                self.store_error(request, request_id, latency_ms, &e);
                Err(e)
            },
        }
    }

    pub async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        tools: &[McpTool],
        context: &RequestContext,
        agent_overrides: Option<&systemprompt_models::ai::ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        self.tooled_executor
            .execute_tool_calls(tool_calls, tools, context, agent_overrides)
            .await
    }

    pub async fn list_available_tools_for_agent(
        &self,
        agent_name: &systemprompt_identifiers::AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        self.tool_discovery
            .discover_tools(agent_name, context)
            .await
    }
}
