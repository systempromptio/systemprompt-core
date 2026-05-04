use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;

use systemprompt_identifiers::AgentName;
use systemprompt_models::RequestContext;
use systemprompt_models::ai::{
    AiProvider, AiRequest, AiResponse, CallToolResult, GenerateResponseParams, GoogleSearchParams,
    McpTool, PlanningResult, SearchGroundedResponse, StreamChunk, ToolCall, ToolModelOverrides,
};
use systemprompt_models::errors::ProviderResult;

use super::service::AiService;

fn boxed_err<E: std::fmt::Display>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::<dyn std::error::Error + Send + Sync>::from(e.to_string())
}

#[async_trait]
impl AiProvider for AiService {
    fn default_provider(&self) -> &str {
        Self::default_provider(self)
    }

    fn default_model(&self) -> &str {
        Self::default_model(self)
    }

    fn default_max_output_tokens(&self) -> u32 {
        Self::default_max_output_tokens(self)
    }

    async fn generate(&self, request: &AiRequest) -> ProviderResult<AiResponse> {
        Self::generate(self, request).await.map_err(boxed_err)
    }

    async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> ProviderResult<Pin<Box<dyn Stream<Item = ProviderResult<StreamChunk>> + Send>>> {
        let stream = Self::generate_stream(self, request)
            .await
            .map_err(boxed_err)?;
        use futures::StreamExt;
        let mapped = stream.map(|item| item.map_err(boxed_err));
        Ok(Box::pin(mapped))
    }

    async fn generate_with_tools(&self, request: &AiRequest) -> ProviderResult<AiResponse> {
        Self::generate_with_tools(self, request)
            .await
            .map_err(boxed_err)
    }

    async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> ProviderResult<Pin<Box<dyn Stream<Item = ProviderResult<StreamChunk>> + Send>>> {
        let stream = Self::generate_with_tools_stream(self, request)
            .await
            .map_err(boxed_err)?;
        use futures::StreamExt;
        let mapped = stream.map(|item| item.map_err(boxed_err));
        Ok(Box::pin(mapped))
    }

    async fn generate_single_turn(
        &self,
        request: &AiRequest,
    ) -> ProviderResult<(AiResponse, Vec<ToolCall>)> {
        Self::generate_single_turn(self, request)
            .await
            .map_err(boxed_err)
    }

    async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        tools: &[McpTool],
        context: &RequestContext,
        agent_overrides: Option<&ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>) {
        Self::execute_tools(self, tool_calls, tools, context, agent_overrides).await
    }

    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> ProviderResult<Vec<McpTool>> {
        Self::list_available_tools_for_agent(self, agent_name, context)
            .await
            .map_err(boxed_err)
    }

    async fn generate_with_google_search(
        &self,
        params: GoogleSearchParams<'_>,
    ) -> ProviderResult<SearchGroundedResponse> {
        Self::generate_with_google_search(self, params)
            .await
            .map_err(boxed_err)
    }

    async fn health_check(&self) -> ProviderResult<HashMap<String, bool>> {
        Self::health_check(self).await.map_err(boxed_err)
    }

    async fn generate_plan(
        &self,
        request: &AiRequest,
        available_tools: &[McpTool],
    ) -> ProviderResult<PlanningResult> {
        Self::generate_plan(self, request, available_tools)
            .await
            .map_err(boxed_err)
    }

    async fn generate_response(
        &self,
        params: GenerateResponseParams<'_>,
    ) -> ProviderResult<String> {
        Self::generate_response(self, params)
            .await
            .map_err(boxed_err)
    }
}
