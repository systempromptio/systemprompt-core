// `ProviderClient::embed` across the three drivers: the OpenAI and Gemini
// request bodies as they leave the process, both reply shapes mapped back to
// one vector per input, the Vertex `:predict` rendering that wiremock cannot
// host (hosting is derived from the host name), and the Anthropic driver
// refusing the capability — through the resilience decorator too.

use std::sync::Arc;

use serde_json::{Value, json};
use systemprompt_ai::error::{AiError, ProviderCapability, ProviderError, RoutingError};
use systemprompt_ai::models::ai::{EmbeddingRequest, EmbeddingUsage};
use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_ai::services::providers::gemini::{GeminiProvider, embeddings as gemini};
use systemprompt_ai::services::providers::openai::{OpenAiProvider, embeddings as openai};
use systemprompt_ai::services::providers::{ProviderClient, ResilientProvider};
use systemprompt_manifest::services::ResilienceSettings;
use systemprompt_wire::{Hosting, WireProtocol};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::services::providers::mock_http;

const OPENAI_MODEL: &str = "text-embedding-3-small";
const GOOGLE_MODEL: &str = "text-embedding-005";

fn request(model: &str) -> EmbeddingRequest {
    EmbeddingRequest::new(model, vec!["alpha".to_owned(), "beta".to_owned()]).with_dimensions(3)
}

async fn mock(route: &str, status: u16, body: Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

async fn sent_body(server: &MockServer) -> Value {
    let received = server.received_requests().await.expect("recorded requests");
    serde_json::from_slice(&received[0].body).expect("json body")
}

fn openai_provider(endpoint: &str) -> OpenAiProvider {
    OpenAiProvider::with_target(mock_http::api_key_target(
        "openai",
        WireProtocol::OpenAiChat,
        endpoint,
        "k",
    ))
}

fn gemini_provider(endpoint: &str) -> GeminiProvider {
    GeminiProvider::with_target(mock_http::api_key_target(
        "gemini",
        WireProtocol::Gemini,
        endpoint,
        "k",
    ))
    .expect("gemini client builds")
}

#[tokio::test]
async fn openai_posts_the_embeddings_body_and_maps_vectors_by_index() {
    let reply = json!({
        "object": "list",
        "model": OPENAI_MODEL,
        "data": [
            { "object": "embedding", "index": 1, "embedding": [0.4, 0.5, 0.6] },
            { "object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3] }
        ],
        "usage": { "prompt_tokens": 4, "total_tokens": 4 }
    });
    let server = mock("/embeddings", 200, reply).await;
    let provider = openai_provider(&server.uri());

    let response = provider
        .embed(&request(OPENAI_MODEL))
        .await
        .expect("embeddings succeed");

    let body = sent_body(&server).await;
    assert_eq!(body["model"], OPENAI_MODEL);
    assert_eq!(body["input"], json!(["alpha", "beta"]));
    assert_eq!(body["dimensions"], 3);
    assert_eq!(body["encoding_format"], "float");
    assert_eq!(
        response.vectors,
        vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]]
    );
    assert_eq!(
        response.usage,
        EmbeddingUsage {
            input_tokens: 4,
            total_tokens: 4
        }
    );
    assert_eq!(response.dimensions(), Some(3));
    assert!(provider.supports_embeddings());
}

#[test]
fn openai_omits_dimensions_when_the_caller_did_not_set_them() {
    let request = EmbeddingRequest::new(OPENAI_MODEL, vec!["x".to_owned()]);
    let body = openai::build_request_body(&request, OPENAI_MODEL).expect("renders");
    assert!(body.get("dimensions").is_none(), "got {body}");
}

#[test]
fn openai_rejects_a_reply_with_the_wrong_number_of_vectors() {
    let reply = json!({ "data": [ { "index": 0, "embedding": [0.1] } ] });
    let err = openai::parse_response(reply, &request(OPENAI_MODEL)).expect_err("mismatch");
    assert!(
        matches!(
            err,
            AiError::Provider(ProviderError::EmbeddingCountMismatch {
                requested: 2,
                returned: 1,
                ..
            })
        ),
        "got {err}"
    );
}

#[tokio::test]
async fn openai_surfaces_an_upstream_failure() {
    let server = mock(
        "/embeddings",
        429,
        json!({"error": {"message": "slow down"}}),
    )
    .await;
    let err = openai_provider(&server.uri())
        .embed(&request(OPENAI_MODEL))
        .await
        .expect_err("a 429 is an error");
    assert!(
        matches!(
            err,
            AiError::Provider(ProviderError::HttpStatus { status: 429, .. })
        ),
        "got {err}"
    );
}

#[tokio::test]
async fn gemini_public_api_uses_batch_embed_contents() {
    let reply = json!({
        "embeddings": [ { "values": [1.0, 2.0, 3.0] }, { "values": [4.0, 5.0, 6.0] } ]
    });
    let route = format!("/models/{GOOGLE_MODEL}:batchEmbedContents");
    let server = mock(&route, 200, reply).await;
    let provider = gemini_provider(&server.uri());

    let response = provider
        .embed(&request(GOOGLE_MODEL))
        .await
        .expect("embeddings succeed");

    let body = sent_body(&server).await;
    let requests = body["requests"].as_array().expect("requests array");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["model"], format!("models/{GOOGLE_MODEL}"));
    assert_eq!(requests[0]["content"]["parts"][0]["text"], "alpha");
    assert_eq!(requests[1]["outputDimensionality"], 3);
    assert_eq!(
        response.vectors,
        vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]
    );
    assert_eq!(response.usage, EmbeddingUsage::default());
    assert!(provider.supports_embeddings());
}

#[test]
fn vertex_renders_predict_instances_and_reads_predictions() {
    let vertex_request = request(GOOGLE_MODEL);
    assert_eq!(
        gemini::embeddings_path(Hosting::Vertex, GOOGLE_MODEL),
        format!("/models/{GOOGLE_MODEL}:predict")
    );
    let body = gemini::build_request_body(Hosting::Vertex, &vertex_request, GOOGLE_MODEL);
    assert_eq!(
        body["instances"],
        json!([{ "content": "alpha" }, { "content": "beta" }])
    );
    assert_eq!(body["parameters"]["outputDimensionality"], 3);

    let reply = json!({
        "predictions": [
            { "embeddings": { "values": [0.1, 0.2], "statistics": { "token_count": 2, "truncated": false } } },
            { "embeddings": { "values": [0.3, 0.4], "statistics": { "token_count": 3, "truncated": false } } }
        ],
        "metadata": { "billableCharacterCount": 9 }
    });
    let response = gemini::parse_response(Hosting::Vertex, reply, &vertex_request).expect("parses");
    assert_eq!(response.vectors, vec![vec![0.1, 0.2], vec![0.3, 0.4]]);
    assert_eq!(response.usage.input_tokens, 5);
    assert_eq!(response.model, GOOGLE_MODEL);
}

#[test]
fn vertex_rejects_a_short_prediction_list() {
    let reply = json!({ "predictions": [ { "embeddings": { "values": [0.1] } } ] });
    let err = gemini::parse_response(Hosting::Vertex, reply, &request(GOOGLE_MODEL))
        .expect_err("mismatch");
    assert!(
        matches!(
            err,
            AiError::Provider(ProviderError::EmbeddingCountMismatch {
                requested: 2,
                returned: 1,
                ..
            })
        ),
        "got {err}"
    );
}

#[tokio::test]
async fn anthropic_has_no_embeddings_api_even_behind_the_resilience_layer() {
    let bare = AnthropicProvider::with_target(mock_http::api_key_target(
        "anthropic",
        WireProtocol::Anthropic,
        "http://localhost:9",
        "k",
    ));
    assert!(!bare.supports_embeddings());
    let err = bare
        .embed(&request("claude-sonnet-5"))
        .await
        .expect_err("anthropic refuses");
    assert!(
        matches!(
            err,
            AiError::Routing(RoutingError::CapabilityUnsupported {
                capability: ProviderCapability::Embeddings,
                ..
            })
        ),
        "got {err}"
    );

    let resilient =
        ResilientProvider::new("anthropic", Arc::new(bare), &ResilienceSettings::default());
    assert!(!resilient.supports_embeddings());
    let err = resilient
        .embed(&request("claude-sonnet-5"))
        .await
        .expect_err("the decorator delegates the refusal");
    assert!(
        matches!(
            err,
            AiError::Routing(RoutingError::CapabilityUnsupported {
                capability: ProviderCapability::Embeddings,
                ..
            })
        ),
        "got {err}"
    );
}
