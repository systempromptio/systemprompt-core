// `generate_with_google_search` on the OpenAI provider — the web-search
// grounded path, which routes through the Responses endpoint rather than chat
// completions and is the only caller of `SearchParams::with_sampling`.

use serde_json::json;
use systemprompt_ai::models::ai::{AiMessage, SamplingParams};
use systemprompt_ai::services::providers::openai::OpenAiProvider;
use systemprompt_ai::services::providers::provider_trait::{
    AiProvider, GenerationParams, SearchGenerationParams,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::services::providers::mock_http;

const MODEL: &str = "gpt-4.1";

fn grounded_body(text: &str) -> serde_json::Value {
    json!({
        "id": "resp_1",
        "object": "response",
        "model": MODEL,
        "status": "completed",
        "output": [
            {
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [
                    {
                        "type": "output_text",
                        "text": text,
                        "annotations": [
                            {
                                "type": "url_citation",
                                "url": "https://example.test/source",
                                "title": "A cited source",
                                "start_index": 0,
                                "end_index": 4
                            }
                        ]
                    }
                ]
            }
        ],
        "usage": { "input_tokens": 11, "output_tokens": 7, "total_tokens": 18 }
    })
}

fn provider(endpoint: String) -> OpenAiProvider {
    OpenAiProvider::with_endpoint("k".to_owned(), endpoint)
        .with_models(mock_http::seed_models("openai"))
}

#[tokio::test]
async fn a_grounded_search_returns_the_answer_and_its_citations() {
    let server = mock_http::openai_responses_success(grounded_body("Grounded answer.")).await;
    let provider = provider(server.uri());
    let messages = vec![AiMessage::user("who won")];

    let grounded = provider
        .generate_with_google_search(SearchGenerationParams {
            base: GenerationParams::new(&messages, MODEL, 128),
            urls: None,
            response_schema: None,
        })
        .await
        .expect("the grounded search succeeds");

    assert!(
        grounded.content.contains("Grounded answer"),
        "the answer text must survive the mapping, got {}",
        grounded.content
    );
    assert!(
        grounded
            .sources
            .iter()
            .any(|s| s.uri.contains("example.test")),
        "the url citation must be carried onto the grounded response, got {:?}",
        grounded.sources
    );
}

#[tokio::test]
async fn sampling_parameters_are_carried_into_the_search_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(grounded_body("Sampled.")))
        .mount(&server)
        .await;

    let provider = provider(server.uri());
    let messages = vec![AiMessage::user("who won")];
    let sampling = SamplingParams {
        temperature: Some(0.25),
        ..SamplingParams::default()
    };

    provider
        .generate_with_google_search(SearchGenerationParams {
            base: GenerationParams::new(&messages, MODEL, 128).with_sampling(&sampling),
            urls: None,
            response_schema: None,
        })
        .await
        .expect("the grounded search succeeds");

    let received = server.received_requests().await.expect("recorded requests");
    let body = String::from_utf8(received[0].body.clone()).expect("utf8 body");
    assert!(
        body.contains("0.25"),
        "a caller-supplied temperature must reach the upstream request, got {body}"
    );
}

#[tokio::test]
async fn an_upstream_failure_on_the_search_endpoint_is_reported_not_swallowed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(json!({"error": {"message": "search backend down"}})),
        )
        .mount(&server)
        .await;

    let provider = provider(server.uri());
    let messages = vec![AiMessage::user("who won")];

    let err = provider
        .generate_with_google_search(SearchGenerationParams {
            base: GenerationParams::new(&messages, MODEL, 128),
            urls: None,
            response_schema: None,
        })
        .await
        .expect_err("a 500 from the search endpoint must not read as an empty answer");
    assert!(
        err.to_string().contains("500") || err.to_string().contains("search backend down"),
        "the upstream status or message must survive, got {err}"
    );
}
