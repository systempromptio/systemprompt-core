// The hardening around `ProviderClient::embed`: an OpenAI reply that names
// an index twice or leaves a slot unfilled is refused, a reply with no usage
// block is priced from an estimate rather than at zero, empty inputs never
// reach the wire, and a request over an endpoint's input cap is split into
// calls and joined back in input order with the usage summed.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use systemprompt_ai::error::{AiError, ProviderError};
use systemprompt_ai::models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};
use systemprompt_ai::services::providers::ProviderClient;
use systemprompt_ai::services::providers::embedding_batches::{
    GEMINI_MAX_BATCH, OPENAI_MAX_INPUTS, VERTEX_MAX_INSTANCES, embed_in_batches, estimated_usage,
};
use systemprompt_ai::services::providers::gemini::{GeminiProvider, embeddings as gemini};
use systemprompt_ai::services::providers::openai::{OpenAiProvider, embeddings as openai};
use systemprompt_wire::{Hosting, WireProtocol};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use crate::services::providers::mock_http;

const OPENAI_MODEL: &str = "text-embedding-3-small";
const GOOGLE_MODEL: &str = "text-embedding-005";

fn request(model: &str, inputs: &[&str]) -> EmbeddingRequest {
    EmbeddingRequest::new(model, inputs.iter().map(|s| (*s).to_owned()).collect())
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

#[test]
fn openai_refuses_a_duplicate_index() {
    let reply = json!({
        "data": [
            { "index": 0, "embedding": [0.1] },
            { "index": 0, "embedding": [0.2] }
        ],
        "usage": { "prompt_tokens": 2, "total_tokens": 2 }
    });
    let err = openai::parse_response(reply, &request(OPENAI_MODEL, &["a", "b"]))
        .expect_err("slot 1 is never filled");
    assert!(
        matches!(
            err,
            AiError::Provider(ProviderError::EmbeddingIndexInvalid { index: 0, .. })
        ),
        "got {err}"
    );
}

#[test]
fn openai_refuses_an_index_past_the_inputs() {
    let reply = json!({
        "data": [
            { "index": 0, "embedding": [0.1] },
            { "index": 7, "embedding": [0.2] }
        ]
    });
    let err = openai::parse_response(reply, &request(OPENAI_MODEL, &["a", "b"]))
        .expect_err("index 7 has no slot");
    assert!(
        matches!(
            err,
            AiError::Provider(ProviderError::EmbeddingIndexInvalid { index: 7, .. })
        ),
        "got {err}"
    );
}

#[test]
fn openai_estimates_usage_when_the_reply_carries_none() {
    let reply = json!({
        "data": [
            { "index": 0, "embedding": [0.1] },
            { "index": 1, "embedding": [0.2] }
        ]
    });
    let inputs = ["twelve chars", "and eight"];
    let response = openai::parse_response(reply, &request(OPENAI_MODEL, &inputs)).expect("parses");
    let expected = estimated_usage(&inputs.map(str::to_owned));
    assert!(expected.estimated);
    assert_eq!(expected.input_tokens, (12 + 9) / 4 + 1);
    assert_eq!(response.usage, expected);
}

#[test]
fn vertex_estimates_usage_when_no_prediction_reports_statistics() {
    let reply = json!({
        "predictions": [
            { "embeddings": { "values": [0.1] } },
            { "embeddings": { "values": [0.2] } }
        ]
    });
    let request = request(GOOGLE_MODEL, &["a", "b"]);
    let response = gemini::parse_response(Hosting::Vertex, reply, &request).expect("parses");
    assert_eq!(response.usage, EmbeddingUsage::from_estimate(1));
}

#[test]
fn provider_caps_follow_the_documented_limits() {
    assert_eq!(OPENAI_MAX_INPUTS, 2048);
    assert_eq!(gemini::max_inputs(Hosting::Vertex), VERTEX_MAX_INSTANCES);
    assert_eq!(gemini::max_inputs(Hosting::FirstParty), GEMINI_MAX_BATCH);
    assert_eq!(VERTEX_MAX_INSTANCES, 250);
    assert_eq!(GEMINI_MAX_BATCH, 100);
}

#[tokio::test]
async fn empty_inputs_never_reach_the_wire() {
    let server = MockServer::start().await;
    let openai = openai_provider(&server.uri());
    let gemini = gemini_provider(&server.uri());

    let from_openai = openai
        .embed(&request(OPENAI_MODEL, &[]))
        .await
        .expect("nothing to embed is not an error");
    let from_gemini = gemini
        .embed(&request(GOOGLE_MODEL, &[]))
        .await
        .expect("nothing to embed is not an error");

    assert_eq!(from_openai, EmbeddingResponse::empty(OPENAI_MODEL.into()));
    assert_eq!(from_gemini, EmbeddingResponse::empty(GOOGLE_MODEL.into()));
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded")
            .is_empty()
    );
}

#[tokio::test]
async fn the_batcher_splits_at_the_cap_and_joins_in_order() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let request = request("m", &["a", "b", "c", "d", "e"]);

    let joined = embed_in_batches(&request, 2, |chunk| {
        let seen = seen.clone();
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            let vectors = chunk
                .inputs
                .iter()
                .map(|text| vec![f32::from(text.as_bytes()[0])])
                .collect();
            Ok(EmbeddingResponse {
                model: chunk.model.clone(),
                vectors,
                usage: EmbeddingUsage::reported(
                    u32::try_from(chunk.inputs.len()).expect("small chunk"),
                    0,
                ),
            })
        }
    })
    .await
    .expect("every chunk succeeds");

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        joined.vectors,
        vec![vec![97.0], vec![98.0], vec![99.0], vec![100.0], vec![101.0]]
    );
    assert_eq!(joined.usage, EmbeddingUsage::reported(5, 0));
}

#[tokio::test]
async fn gemini_public_api_sends_more_than_one_hundred_inputs_as_two_calls() {
    let inputs: Vec<String> = (0..101).map(|i| format!("text {i}")).collect();
    let server = MockServer::start().await;
    let route = format!("/models/{GOOGLE_MODEL}:batchEmbedContents");
    Mock::given(method("POST"))
        .and(path(route))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).expect("json body");
            let count = body["requests"].as_array().map_or(0, Vec::len);
            let embeddings: Vec<Value> = (0..count).map(|_| json!({ "values": [1.0] })).collect();
            ResponseTemplate::new(200).set_body_json(json!({ "embeddings": embeddings }))
        })
        .mount(&server)
        .await;
    let provider = gemini_provider(&server.uri());

    let response = provider
        .embed(&EmbeddingRequest::new(GOOGLE_MODEL, inputs.clone()))
        .await
        .expect("both calls succeed");

    let received = server.received_requests().await.expect("recorded");
    let sizes: Vec<usize> = received
        .iter()
        .map(|r| {
            let body: Value = serde_json::from_slice(&r.body).expect("json body");
            body["requests"].as_array().map_or(0, Vec::len)
        })
        .collect();
    assert_eq!(sizes, vec![100, 1]);
    assert_eq!(response.vectors.len(), 101);
    assert!(response.usage.estimated);
    let expected = estimated_usage(&inputs[..100]).summed_with(estimated_usage(&inputs[100..]));
    assert_eq!(response.usage, expected);
}
