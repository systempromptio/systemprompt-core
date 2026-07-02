// Async drivers for `ResponseSynthesizer::synthesize_or_fallback`, backed by a
// configurable in-test `AiProvider` stub. These cover the three terminal
// branches in `tooled/synthesizer.rs`: success from the tool-results call,
// success from the explicit guidance prompt, the empty-content fallback, and
// the provider-error fallback.

use async_trait::async_trait;
use rmcp::model::ContentBlock;
use serde_json::json;
use std::any::Any;
use std::sync::Mutex;
use systemprompt_ai::error::{AiError, Result};
use systemprompt_ai::models::ai::{AiMessage, AiResponse, MessageRole};
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, SchemaGenerationParams, ToolGenerationParams, ToolResultsParams,
};
use systemprompt_ai::services::schema::ProviderCapabilities;
use systemprompt_ai::services::tooled::{ResponseSynthesizer, SynthesisParams};
use systemprompt_identifiers::AiToolCallId;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
enum Outcome {
    Text(&'static str),
    Empty,
    Error,
}

struct StubProvider {
    tool_results_outcome: Outcome,
    generate_outcome: Outcome,
    generate_calls: Mutex<usize>,
}

impl StubProvider {
    fn new(tool_results_outcome: Outcome, generate_outcome: Outcome) -> Self {
        Self {
            tool_results_outcome,
            generate_outcome,
            generate_calls: Mutex::new(0),
        }
    }

    fn response(text: &str) -> AiResponse {
        let mut resp = AiResponse::default();
        resp.request_id = Uuid::new_v4();
        resp.content = text.to_owned();
        resp.provider = "stub".to_owned();
        resp.model = "stub-model".to_owned();
        resp
    }

    fn resolve(outcome: Outcome) -> Result<AiResponse> {
        match outcome {
            Outcome::Text(text) => Ok(Self::response(text)),
            Outcome::Empty => Ok(Self::response("")),
            Outcome::Error => Err(AiError::Internal("stub failure".to_owned())),
        }
    }
}

#[async_trait]
impl AiProvider for StubProvider {
    fn name(&self) -> &str {
        "stub"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::gemini()
    }

    fn supports_model(&self, _model: &str) -> bool {
        true
    }

    fn supports_sampling(
        &self,
        _sampling: Option<&systemprompt_ai::models::ai::SamplingParams>,
    ) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        "stub-model"
    }

    fn get_pricing(&self, _model: &str) -> systemprompt_ai::services::providers::ModelPricing {
        systemprompt_ai::services::providers::ModelPricing::default()
    }

    async fn generate(&self, _params: GenerationParams<'_>) -> Result<AiResponse> {
        *self.generate_calls.lock().expect("lock") += 1;
        Self::resolve(self.generate_outcome)
    }

    async fn generate_with_tools(
        &self,
        _params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        Ok((Self::response("tools"), Vec::new()))
    }

    async fn generate_with_tool_results(
        &self,
        _params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        Self::resolve(self.tool_results_outcome)
    }

    async fn generate_with_schema(
        &self,
        _params: SchemaGenerationParams<'_>,
    ) -> Result<AiResponse> {
        Self::resolve(self.generate_outcome)
    }
}

fn original_messages() -> Vec<AiMessage> {
    vec![AiMessage {
        role: MessageRole::User,
        content: "what is the weather?".to_owned(),
        parts: Vec::new(),
    }]
}

fn tool_calls() -> Vec<ToolCall> {
    vec![ToolCall {
        ai_tool_call_id: AiToolCallId::new("call-1"),
        name: "get_weather".to_owned(),
        arguments: json!({ "city": "Paris" }),
    }]
}

fn tool_results() -> Vec<CallToolResult> {
    vec![CallToolResult::success(vec![ContentBlock::text(
        "sunny, 24C",
    )])]
}

async fn run(provider: &StubProvider) -> String {
    let messages = original_messages();
    let calls = tool_calls();
    let results = tool_results();
    let synthesizer = ResponseSynthesizer::new();
    let params = SynthesisParams {
        provider,
        original_messages: &messages,
        tool_calls: &calls,
        tool_results: &results,
        sampling: None,
        max_output_tokens: 256,
        model: "stub-model",
    };
    synthesizer.synthesize_or_fallback(params).await
}

#[tokio::test]
async fn tool_results_success_is_returned_directly() {
    let provider = StubProvider::new(Outcome::Text("Tool-results synthesis."), Outcome::Error);
    let out = run(&provider).await;

    assert_eq!(out, "Tool-results synthesis.");
    assert_eq!(*provider.generate_calls.lock().expect("lock"), 0);
}

#[tokio::test]
async fn empty_tool_results_falls_back_to_guidance_generate() {
    let provider = StubProvider::new(Outcome::Empty, Outcome::Text("Guidance synthesis."));
    let out = run(&provider).await;

    assert_eq!(out, "Guidance synthesis.");
    assert_eq!(*provider.generate_calls.lock().expect("lock"), 1);
}

#[tokio::test]
async fn tool_results_error_falls_back_to_guidance_generate() {
    let provider = StubProvider::new(Outcome::Error, Outcome::Text("Recovered via guidance."));
    let out = run(&provider).await;

    assert_eq!(out, "Recovered via guidance.");
    assert_eq!(*provider.generate_calls.lock().expect("lock"), 1);
}

#[tokio::test]
async fn empty_content_everywhere_produces_empty_content_fallback() {
    let provider = StubProvider::new(Outcome::Empty, Outcome::Empty);
    let out = run(&provider).await;

    assert!(out.contains("Tool execution completed"));
    assert!(!out.contains("Synthesis error"));
    assert!(out.contains("get_weather"));
}

#[tokio::test]
async fn provider_error_produces_synthesis_failed_fallback() {
    let provider = StubProvider::new(Outcome::Error, Outcome::Error);
    let out = run(&provider).await;

    assert!(out.contains("Tool execution completed"));
    assert!(out.contains("Synthesis error"));
    assert!(out.contains("stub failure"));
}
