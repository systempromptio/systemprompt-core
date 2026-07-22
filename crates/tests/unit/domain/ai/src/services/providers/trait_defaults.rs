// Exercises the default method bodies on `AiProvider` through a stub that
// implements only the required methods, plus the structured-output retry
// driver.

use async_trait::async_trait;
use rmcp::model::ContentBlock;
use serde_json::json;
use std::any::Any;
use std::sync::Mutex;
use systemprompt_ai::error::{AiError, Result};
use systemprompt_ai::models::ai::{
    AiMessage, AiResponse, MessageRole, ResponseFormat, SamplingParams, StructuredOutputOptions,
};
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, ToolResultsParams,
};
use systemprompt_ai::services::schema::ProviderCapabilities;
use systemprompt_ai::services::structured_output::StructuredOutputProcessor;
use systemprompt_identifiers::AiToolCallId;

#[derive(Default)]
struct MinimalProvider {
    last_prompt: Mutex<Option<String>>,
}

impl MinimalProvider {
    fn response(text: &str) -> AiResponse {
        let mut resp = AiResponse::default();
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

    fn as_any(&self) -> &dyn Any {
        self
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

    fn get_pricing(&self, _model: &str) -> ModelPricing {
        ModelPricing::default()
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
    assert!(provider.supports_structured_output());
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

fn options() -> StructuredOutputOptions {
    StructuredOutputOptions {
        max_retries: Some(2),
        ..Default::default()
    }
}

#[tokio::test]
async fn retry_returns_first_valid_json() {
    let calls = Mutex::new(0u32);
    let result = StructuredOutputProcessor::generate_with_retry(
        || {
            *calls.lock().expect("lock") += 1;
            async { Ok(r#"{"a": 1}"#.to_owned()) }
        },
        &ResponseFormat::JsonObject,
        &options(),
    )
    .await
    .expect("valid json");
    assert_eq!(result["a"], 1);
    assert_eq!(*calls.lock().expect("lock"), 1);
}

#[tokio::test]
async fn retry_recovers_after_invalid_payload() {
    let calls = Mutex::new(0u32);
    let result = StructuredOutputProcessor::generate_with_retry(
        || {
            let n = {
                let mut guard = calls.lock().expect("lock");
                *guard += 1;
                *guard
            };
            async move {
                if n == 1 {
                    Ok("not json at all".to_owned())
                } else {
                    Ok(r#"{"ok": true}"#.to_owned())
                }
            }
        },
        &ResponseFormat::JsonObject,
        &options(),
    )
    .await
    .expect("second attempt succeeds");
    assert_eq!(result["ok"], true);
    assert_eq!(*calls.lock().expect("lock"), 2);
}

#[tokio::test]
async fn retry_exhaustion_surfaces_last_parse_error() {
    let calls = Mutex::new(0u32);
    let err = StructuredOutputProcessor::generate_with_retry(
        || {
            *calls.lock().expect("lock") += 1;
            async { Ok("still not json".to_owned()) }
        },
        &ResponseFormat::JsonObject,
        &options(),
    )
    .await
    .expect_err("all attempts invalid");
    assert!(!err.to_string().is_empty());
    assert_eq!(
        *calls.lock().expect("lock"),
        3,
        "max_retries=2 means 3 attempts"
    );
}

#[tokio::test]
async fn retry_exhaustion_surfaces_generator_error() {
    let err = StructuredOutputProcessor::generate_with_retry(
        || async { Err(AiError::Internal("provider down".to_owned())) },
        &ResponseFormat::JsonObject,
        &options(),
    )
    .await
    .expect_err("generator always fails");
    assert!(matches!(err, AiError::Internal(msg) if msg.contains("provider down")));
}
