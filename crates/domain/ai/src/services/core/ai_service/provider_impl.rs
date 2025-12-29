use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;

use systemprompt_identifiers::AgentName;
use systemprompt_models::ai::{
    AiProvider, AiRequest, AiResponse, CallToolResult, GenerateResponseParams, GoogleSearchParams,
    McpTool, PlanningResult, SearchGroundedResponse, ToolCall, ToolModelOverrides,
};
use systemprompt_models::RequestContext;

use super::service::AiService;

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

    async fn generate(&self, request: &AiRequest) -> Result<AiResponse> {
        Self::generate(self, request).await
    }

    async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        Self::generate_stream(self, request).await
    }

    async fn generate_with_tools(&self, request: &AiRequest) -> Result<AiResponse> {
        Self::generate_with_tools(self, request).await
    }

    async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        Self::generate_with_tools_stream(self, request).await
    }

    async fn generate_single_turn(
        &self,
        request: &AiRequest,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        Self::generate_single_turn(self, request).await
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
    ) -> Result<Vec<McpTool>> {
        Self::list_available_tools_for_agent(self, agent_name, context).await
    }

    async fn generate_with_google_search(
        &self,
        params: GoogleSearchParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        Self::generate_with_google_search(self, params).await
    }

    async fn health_check(&self) -> Result<HashMap<String, bool>> {
        Self::health_check(self).await
    }

    async fn generate_plan(
        &self,
        request: &AiRequest,
        available_tools: &[McpTool],
    ) -> Result<PlanningResult> {
        Self::generate_plan(self, request, available_tools).await
    }

    async fn generate_response(&self, params: GenerateResponseParams<'_>) -> Result<String> {
        Self::generate_response(self, params).await
    }
}
