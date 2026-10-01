// Exercises the default method bodies on `AiProvider` through a stub that
// implements only the required methods.

use async_trait::async_trait;
use rmcp::model::ContentBlock;
use serde_json::json;
use std::sync::Mutex;
use systemprompt_ai::error::{AiError, Result};
use systemprompt_ai::models::ai::{
    AiMessage, AiResponse, MessageRole, ResponseFormat, SamplingParams,
};
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, ToolResultsParams,
};
use systemprompt_ai::services::schema::ProviderCapabilities;
use systemprompt_identifiers::{AiRequestId, AiToolCallId};

#[derive(Default)]
struct MinimalProvider {
    last_prompt: Mutex<Option<String>>,
}

impl MinimalProvider {
    fn response(text: &str) -> AiResponse {
        let mut resp = AiResponse::new(
            AiRequestId::generate(),
            String::new(),
            String::new(),
            String::new(),
        );
        resp.content = text.to_owned();
        resp.provider = "minimal".to_owned();
        resp.model = "minimal-model".to_owned();
        resp
    }
}

#[async_trait]
impl AiProvider for MinimalProvider {
    fn name(&self) -> &str {
        "minimal"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::gemini()
    }

    fn supports_model(&self, _model: &str) -> bool {
        true
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        "minimal-model"
    }

    fn get_pricing(&self, _model: &str) -> Option<ModelPricing> {
        Some(ModelPricing::default())
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        let prompt = params
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        *self.last_prompt.lock().expect("lock") = Some(prompt);
        Ok(Self::response("generated"))
    }

    async fn generate_with_tools(
        &self,
        _params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        Ok((Self::response("tools"), Vec::new()))
    }

    async fn generate_with_schema(
        &self,
        _params: SchemaGenerationParams<'_>,
    ) -> Result<AiResponse> {
        Ok(Self::response("schema"))
    }
}

fn messages() -> Vec<AiMessage> {
    vec![AiMessage {
        role: MessageRole::User,
        content: "original question".to_owned(),
        parts: Vec::new(),
    }]
}

fn text_block(text: &str) -> ContentBlock {
    ContentBlock::text(text)
}

#[tokio::test]
async fn default_tool_results_summarises_success_and_failure_into_prompt() {
    let provider = MinimalProvider::default();
    let msgs = messages();
    let calls = vec![
        ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-1"),
            name: "search".to_owned(),
            arguments: json!({}),
        },
        ToolCall {
            ai_tool_call_id: AiToolCallId::new("call-2"),
            name: "fetch".to_owned(),
            arguments: json!({}),
        },
    ];
    let results = vec![
        CallToolResult::success(vec![text_block("found it")]),
        CallToolResult::error(vec![text_block("boom")]),
    ];

    let params = ToolResultsParams::new(
        GenerationParams::new(&msgs, "minimal-model", 512),
        &calls,
        &results,
    );
    let response = provider
        .generate_with_tool_results(params)
        .await
        .expect("delegates to generate");
    assert_eq!(response.content, "generated");

    let prompt = provider
        .last_prompt
        .lock()
        .expect("lock")
        .clone()
        .expect("prompt captured");
    assert!(prompt.contains("Tool search result: found it"));
    assert!(prompt.contains("Tool fetch failed: boom"));
}

#[tokio::test]
async fn default_generate_structured_delegates_to_generate() {
    let provider = MinimalProvider::default();
    let msgs = messages();
    let format = ResponseFormat::JsonObject;
    let params = StructuredGenerationParams::new(
        GenerationParams::new(&msgs, "minimal-model", 512),
        &format,
    );

    let response = provider.generate_structured(params).await.expect("ok");
    assert_eq!(response.content, "generated");
}

#[tokio::test]
async fn default_capability_flags_and_unsupported_operations() {
    let provider = MinimalProvider::default();
    assert!(!provider.supports_json_mode());
    assert!(
        !provider.supports_structured_output(),
        "the default generate_structured drops the response format, so the default must not \
         advertise structured output"
    );
    assert!(!provider.supports_streaming());
    assert!(!provider.supports_google_search());

    let msgs = messages();
    let stream = provider
        .generate_stream(GenerationParams::new(&msgs, "m", 64))
        .await;
    assert!(matches!(stream, Err(AiError::Internal(msg)) if msg.contains("minimal")));

    let tool_stream = provider
        .generate_with_tools_stream(ToolGenerationParams::new(
            GenerationParams::new(&msgs, "m", 64),
            Vec::new(),
        ))
        .await;
    assert!(matches!(tool_stream, Err(AiError::Internal(_))));

    let search = provider
        .generate_with_google_search(SearchGenerationParams::new(GenerationParams::new(
            &msgs, "m", 64,
        )))
        .await;
    assert!(matches!(search, Err(AiError::Internal(msg)) if msg.contains("Google Search")));
}
