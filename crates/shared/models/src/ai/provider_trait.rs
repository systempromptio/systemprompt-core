use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use super::execution_plan::PlanningResult;
use super::request::{AiMessage, AiRequest};
use super::response::{AiResponse, SearchGroundedResponse};
use super::sampling::SamplingParams;
use super::tools::{CallToolResult, McpTool, ToolCall};
use crate::execution::context::RequestContext;
use systemprompt_identifiers::AgentName;

#[derive(Debug)]
pub struct GenerateResponseParams<'a> {
    pub messages: Vec<AiMessage>,
    pub execution_summary: &'a str,
    pub context: &'a RequestContext,
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug)]
pub struct GoogleSearchParams<'a> {
    pub messages: Vec<AiMessage>,
    pub sampling: Option<SamplingParams>,
    pub max_output_tokens: u32,
    pub model: Option<&'a str>,
    pub urls: Option<Vec<String>>,
    pub response_schema: Option<serde_json::Value>,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn default_provider(&self) -> &str;

    fn default_model(&self) -> &str;

    fn default_max_output_tokens(&self) -> u32;

    async fn generate(&self, request: &AiRequest) -> Result<AiResponse>;

    async fn generate_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;

    async fn generate_with_tools(&self, request: &AiRequest) -> Result<AiResponse>;

    async fn generate_with_tools_stream(
        &self,
        request: &AiRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;

    async fn generate_single_turn(
        &self,
        request: &AiRequest,
    ) -> Result<(AiResponse, Vec<ToolCall>)>;

    async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        tools: &[McpTool],
        context: &RequestContext,
        agent_overrides: Option<&super::ToolModelOverrides>,
    ) -> (Vec<ToolCall>, Vec<CallToolResult>);

    async fn list_available_tools_for_agent(
        &self,
        agent_name: &AgentName,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>>;

    async fn generate_with_google_search(
        &self,
        params: GoogleSearchParams<'_>,
    ) -> Result<SearchGroundedResponse>;

    async fn health_check(&self) -> Result<HashMap<String, bool>>;

    async fn generate_plan(
        &self,
        request: &AiRequest,
        available_tools: &[McpTool],
    ) -> Result<PlanningResult>;

    async fn generate_response(&self, params: GenerateResponseParams<'_>) -> Result<String>;
}

pub type DynAiProvider = Arc<dyn AiProvider>;
