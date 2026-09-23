use async_trait::async_trait;
use futures::{StreamExt, stream};
use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::services::providers::mock_http;
use systemprompt_ai::error::{AiError, Result};
use systemprompt_ai::models::ai::{
    AiMessage, AiResponse, ResponseFormat, SamplingParams, StreamChunk,
};
use systemprompt_ai::models::tools::{McpTool, ToolCall};
use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_ai::services::providers::resilient_provider::ResilientProvider;
use systemprompt_ai::services::providers::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, StructuredGenerationParams,
    ToolGenerationParams, ToolResultsParams,
};
use systemprompt_ai::services::schema::ProviderCapabilities;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::services::{ResilienceSettings, WireProtocol};

fn settings() -> ResilienceSettings {
    ResilienceSettings::default()
}

fn anthropic(endpoint: &str) -> AnthropicProvider {
    AnthropicProvider::with_target(mock_http::api_key_target(
        "anthropic",
        WireProtocol::Anthropic,
        endpoint,
        "k",
    ))
}

#[tokio::test]
async fn delegates_generate_to_inner() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("ok via guard"))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let resilient: Arc<dyn AiProvider> =
        Arc::new(ResilientProvider::new("anthropic", Arc::new(inner), &s));

    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-5", 32);
    let resp = resilient.generate(params).await.expect("ok");
    assert!(resp.content.contains("ok via guard"));
}

#[tokio::test]
async fn delegates_metadata() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    assert_eq!(r.name(), "anthropic");
    assert!(r.supports_streaming());
    assert!(r.supports_model("claude-sonnet-5"));
    assert!(!r.supports_model("nope"));
    assert_eq!(r.default_model(), "claude-sonnet-5");
    let _ = r.get_pricing("claude-sonnet-5");
    let _ = r.capabilities();
    let _ = r.supports_json_mode();
    let _ = r.supports_structured_output();
    let _ = r.supports_google_search();
    let _ = r.supports_sampling(None);
    let _ = r.as_any();
}

#[tokio::test]
async fn maps_inner_error() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-5", 16);
    let res = r.generate(params).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn delegates_generate_with_tools() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "f",
        serde_json::json!({}),
    ))
    .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let tools = vec![McpTool::new(
        "f",
        McpServerId::try_new("svc").expect("valid McpServerId"),
    )];
    let params = ToolGenerationParams::new(
        GenerationParams::new(&messages, "claude-sonnet-5", 16),
        tools,
    );
    let (resp, _calls) = r.generate_with_tools(params).await.expect("ok");
    assert!(resp.content.contains("calling tool"));
}

#[tokio::test]
async fn delegates_generate_with_schema() {
    let server = mock_http::anthropic_messages_success(mock_http::anthropic_tool_use_body(
        "structured_output",
        serde_json::json!({"answer": 42}),
    ))
    .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-5", 32);
    let params = SchemaGenerationParams::new(
        base,
        serde_json::json!({"type": "object", "properties": {"answer": {"type": "number"}}}),
    );
    let resp = r.generate_with_schema(params).await.expect("ok");
    assert!(resp.content.contains("42"));
}

#[tokio::test]
async fn delegates_generate_structured() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("plain")).await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-5", 32);
    let fmt = ResponseFormat::json_object();
    let params = StructuredGenerationParams::new(base, &fmt);
    let resp = r.generate_structured(params).await.expect("ok");
    assert!(resp.content.contains("plain"));
}

#[tokio::test]
async fn delegates_generate_with_tool_results() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("after-tool"))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-5", 32);
    let calls: Vec<systemprompt_ai::models::tools::ToolCall> = Vec::new();
    let results: Vec<systemprompt_ai::models::tools::CallToolResult> = Vec::new();
    let params = ToolResultsParams::new(base, &calls, &results);
    let resp = r.generate_with_tool_results(params).await.expect("ok");
    assert!(resp.content.contains("after-tool"));
}

#[tokio::test]
async fn delegates_generate_with_tools_stream() {
    let sse = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"\
               text_delta\",\"text\":\"hi\"}}\n\n";
    let server = mock_http::anthropic_messages_stream(sse).await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let base = GenerationParams::new(&messages, "claude-sonnet-5", 16);
    let tools = vec![McpTool::new(
        "f",
        McpServerId::try_new("svc").expect("valid McpServerId"),
    )];
    let params = ToolGenerationParams::new(base, tools);
    drop(
        r.generate_with_tools_stream(params)
            .await
            .expect("ok stream"),
    );
}

#[tokio::test]
async fn stream_open_failure_releases_permit() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-5", 16);
    let res = r.generate_stream(params).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn stream_call_guards_path() {
    let sse = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"\
               text_delta\",\"text\":\"hi\"}}\n\n";
    let server = mock_http::anthropic_messages_stream(sse).await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];
    let params = GenerationParams::new(&messages, "claude-sonnet-5", 16);
    drop(r.generate_stream(params).await.expect("ok stream"));
}

#[tokio::test]
async fn a_tripped_breaker_reports_circuit_open_instead_of_the_inner_error() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = ResilienceSettings {
        retry_attempts: 1,
        breaker_failure_threshold: 1,
        breaker_open_cooldown_ms: 60_000,
        ..ResilienceSettings::default()
    };
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];

    let first = r
        .generate(GenerationParams::new(&messages, "claude-sonnet-5", 16))
        .await
        .expect_err("upstream 500 must surface as an error");
    assert!(
        !first.to_string().contains("circuit"),
        "the first failure is the inner error, not a breaker trip: {first}"
    );

    let second = r
        .generate(GenerationParams::new(&messages, "claude-sonnet-5", 16))
        .await
        .expect_err("the breaker must now be open");
    let message = second.to_string();
    assert!(
        message.contains("anthropic"),
        "the breaker error must name the provider it protects, got {message}"
    );
    assert!(
        message.to_lowercase().contains("circuit"),
        "a call past the failure threshold must be rejected by the breaker, got {message}"
    );
}

#[tokio::test]
async fn an_open_breaker_also_short_circuits_the_streaming_path() {
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let inner = anthropic(&server.uri()).with_models(mock_http::seed_models("anthropic"));
    let s = ResilienceSettings {
        retry_attempts: 1,
        breaker_failure_threshold: 1,
        breaker_open_cooldown_ms: 60_000,
        ..ResilienceSettings::default()
    };
    let r = ResilientProvider::new("anthropic", Arc::new(inner), &s);
    let messages = vec![AiMessage::user("hi")];

    assert!(
        r.generate_stream(GenerationParams::new(&messages, "claude-sonnet-5", 16))
            .await
            .is_err(),
        "the upstream failure trips the breaker"
    );

    let Err(err) = r
        .generate_stream(GenerationParams::new(&messages, "claude-sonnet-5", 16))
        .await
    else {
        panic!("the open breaker must refuse to open a second stream");
    };
    assert!(
        err.to_string().to_lowercase().contains("circuit"),
        "the streaming path must go through the same permit gate, got {err}"
    );
}

#[tokio::test]
async fn the_debug_rendering_names_the_provider_without_leaking_the_inner_client() {
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let inner = AnthropicProvider::with_target(mock_http::api_key_target(
        "anthropic",
        WireProtocol::Anthropic,
        &server.uri(),
        "secret-api-key",
    ))
    .with_models(mock_http::seed_models("anthropic"));
    let s = settings();
    let rendered = format!(
        "{:?}",
        ResilientProvider::new("anthropic", Arc::new(inner), &s)
    );

    assert!(rendered.contains("anthropic"));
    assert!(
        !rendered.contains("secret-api-key"),
        "the wrapper must not render the inner provider's credentials: {rendered}"
    );
}

struct SequencedStreamProvider {
    opens: AtomicUsize,
}

#[async_trait]
impl AiProvider for SequencedStreamProvider {
    fn name(&self) -> &str {
        "sequenced"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::anthropic()
    }
    fn supports_model(&self, _: &str) -> bool {
        true
    }
    fn supports_sampling(&self, _: Option<&SamplingParams>) -> bool {
        true
    }
    fn default_model(&self) -> &str {
        "sequenced-model"
    }
    fn get_pricing(&self, _: &str) -> Option<ModelPricing> {
        Some(ModelPricing::default())
    }
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn generate(&self, _: GenerationParams<'_>) -> Result<AiResponse> {
        Ok(AiResponse::default())
    }

    async fn generate_with_tools(
        &self,
        _: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        Ok((AiResponse::default(), Vec::new()))
    }

    async fn generate_with_schema(&self, _: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        Ok(AiResponse::default())
    }

    async fn generate_stream(
        &self,
        _: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<StreamChunk>> + Send>>> {
        let sequence = self.opens.fetch_add(1, Ordering::SeqCst);
        if sequence == 0 {
            Ok(Box::pin(stream::pending()))
        } else {
            Ok(Box::pin(stream::iter([Ok(StreamChunk::Text(
                "recovered".to_owned(),
            ))])))
        }
    }
}

#[tokio::test(start_paused = true)]
async fn idle_stream_timeout_releases_the_provider_bulkhead_for_a_later_stream() {
    let settings = ResilienceSettings {
        stream_idle_timeout_ms: 20,
        max_concurrent: 1,
        ..ResilienceSettings::default()
    };
    let inner = Arc::new(SequencedStreamProvider {
        opens: AtomicUsize::new(0),
    });
    let provider = ResilientProvider::new("sequenced", inner.clone(), &settings);
    let messages = vec![AiMessage::user("wait")];

    let stream = provider
        .generate_stream(GenerationParams::new(&messages, "sequenced-model", 16))
        .await
        .expect("the first stream opens");
    let timed_out = tokio::spawn(async move {
        let mut stream = stream;
        let item = stream.next().await.expect("idle timeout item");
        (stream, item)
    });
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(21)).await;
    let (mut first_stream, error) = timed_out.await.expect("stream task");
    let error = error.expect_err("stream must time out");
    assert!(matches!(
        error,
        AiError::Timeout {
            ref provider,
            after_ms: 20
        } if provider == "sequenced"
    ));

    let mut recovered = provider
        .generate_stream(GenerationParams::new(&messages, "sequenced-model", 16))
        .await
        .expect("the timeout must release the sole bulkhead permit");
    assert!(matches!(
        recovered.next().await,
        Some(Ok(StreamChunk::Text(ref text))) if text == "recovered"
    ));
    assert!(
        first_stream.next().await.is_none(),
        "a timed-out stream is terminal even while its handle remains alive"
    );
    assert_eq!(inner.opens.load(Ordering::SeqCst), 2);
}
