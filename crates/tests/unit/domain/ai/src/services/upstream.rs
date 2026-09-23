//! The upstream seam the in-process AI service shares with the gateway.
//!
//! Pins that a catalog entry resolves to the same place whichever stack sends
//! it: an API key stays an API key on the first-party endpoint, and a Google
//! service account on Vertex AI fills `{project}`, mints a bearer and renders
//! the Vertex envelope. Also pins the two driver defects the old per-client
//! plumbing carried: the catalog's upstream id was never sent, and an
//! `openai-responses` provider was posted to `/chat/completions`.

use serde_json::json;
use systemprompt_ai::models::ai::AiMessage;
use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_ai::services::providers::openai::OpenAiProvider;
use systemprompt_ai::services::providers::provider_trait::{AiProvider, GenerationParams};
use systemprompt_ai::{UpstreamTarget, UpstreamTargetError};
use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_models::services::{Hosting, ProviderEntry, ProviderRegistry, WireProtocol};
use systemprompt_test_fixtures::keys::test_key;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::services::providers::mock_http;

const VERTEX_TEMPLATE: &str =
    "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/anthropic";

fn seed(provider: &str) -> ProviderEntry {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(provider)
        .expect("provider present in default catalog")
        .clone()
}

fn service_account(token_uri: &str) -> String {
    json!({
        "type": "service_account",
        "project_id": "fixture-project",
        "client_email": "fixture@example.invalid",
        "private_key": test_key(1).to_pkcs8_pem().unwrap(),
        "token_uri": token_uri,
    })
    .to_string()
}

fn vertex_entry(secret: &str) -> ProviderEntry {
    let mut entry = seed("anthropic");
    entry.endpoint = VERTEX_TEMPLATE.to_owned();
    entry.api_key_secret = SecretName::new(secret);
    entry
}

#[tokio::test]
async fn an_api_key_stays_first_party() {
    let target = UpstreamTarget::resolve(&seed("anthropic"), "sk-fixture").expect("resolves");
    assert_eq!(target.hosting(), Hosting::FirstParty);
    assert_eq!(target.wire(), WireProtocol::Anthropic);

    let call = target.call().await.expect("api key needs no mint");
    assert!(!call.is_bearer());
    let headers = call.headers(WireProtocol::Anthropic);
    assert!(headers.contains(&("x-api-key".to_owned(), "sk-fixture".to_owned())));
    assert!(headers.contains(&("anthropic-version".to_owned(), "2023-06-01".to_owned())));
    assert!(
        call.url(WireProtocol::Anthropic, "claude-sonnet-5", true)
            .ends_with("/v1/messages")
    );
}

#[tokio::test]
async fn a_service_account_reaches_claude_on_vertex() {
    let google = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "access_token": "ya29.minted", "expires_in": 3600 })),
        )
        .expect(1)
        .mount(&google)
        .await;

    let entry = vertex_entry("upstream-seam-vertex-sa");
    let secret = service_account(&format!("{}/token", google.uri()));
    let target = UpstreamTarget::resolve(&entry, &secret).expect("service account resolves");
    assert_eq!(target.hosting(), Hosting::Vertex);
    assert_eq!(
        target.endpoint(),
        "https://aiplatform.googleapis.com/v1/projects/fixture-project/locations/global/publishers/anthropic"
    );

    let call = target.call().await.expect("token minted");
    assert!(call.is_bearer());
    let headers = call.headers(WireProtocol::Anthropic);
    assert!(headers.contains(&("authorization".to_owned(), "Bearer ya29.minted".to_owned())));
    assert!(!headers.iter().any(|(name, _)| name == "x-api-key"));
    assert!(!headers.iter().any(|(name, _)| name == "anthropic-version"));
    assert!(
        call.url(WireProtocol::Anthropic, "claude-sonnet-5", true)
            .ends_with("/publishers/anthropic/models/claude-sonnet-5:streamRawPredict")
    );
}

#[test]
fn a_malformed_service_account_is_refused_at_resolve() {
    let err = UpstreamTarget::resolve(
        &vertex_entry("upstream-seam-malformed"),
        r#"{"type":"service_account"}"#,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        UpstreamTargetError::MalformedCredential { .. }
    ));
}

#[test]
fn a_vertex_template_without_a_project_is_refused_at_resolve() {
    let mut entry = vertex_entry("upstream-seam-no-project");
    entry.wire = WireProtocol::Gemini;
    let err = UpstreamTarget::resolve(&entry, "sk-plain").unwrap_err();
    assert!(matches!(err, UpstreamTargetError::Endpoint { .. }));
}

#[test]
fn an_api_key_is_refused_for_claude_on_vertex() {
    let err =
        UpstreamTarget::resolve(&vertex_entry("upstream-seam-api-key"), "sk-plain").unwrap_err();
    match err {
        UpstreamTargetError::ApiKeyOnVertex { provider, secret } => {
            assert_eq!(provider, "anthropic");
            assert_eq!(secret, "upstream-seam-api-key");
        },
        other => panic!("expected ApiKeyOnVertex, got {other:?}"),
    }
}

#[tokio::test]
async fn the_anthropic_driver_sends_the_catalog_upstream_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_partial_json(
            json!({ "model": "claude-upstream-fixture" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_u",
            "type": "message",
            "role": "assistant",
            "model": "claude-upstream-fixture",
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut models = mock_http::seed_models("anthropic");
    let requested = models[0].id.as_str().to_owned();
    models[0].upstream_model = Some("claude-upstream-fixture".to_owned());
    let provider = AnthropicProvider::with_target(UpstreamTarget::api_key(
        ProviderId::new("anthropic"),
        WireProtocol::Anthropic,
        server.uri(),
        "sk-fixture",
    ))
    .with_models(models);

    let messages = vec![AiMessage::user("hi")];
    provider
        .generate(GenerationParams::new(&messages, &requested, 32))
        .await
        .expect("the upstream id is what the upstream receives");
}

#[tokio::test]
async fn an_openai_responses_provider_posts_to_responses() {
    let server = mock_http::openai_responses_success(json!({
        "id": "resp_1",
        "object": "response",
        "model": "o4-mini",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{ "type": "output_text", "text": "answered", "annotations": [] }]
        }],
        "usage": { "input_tokens": 3, "output_tokens": 1, "total_tokens": 4 }
    }))
    .await;

    let provider = OpenAiProvider::with_target(UpstreamTarget::api_key(
        ProviderId::new("openai-responses"),
        WireProtocol::OpenAiResponses,
        server.uri(),
        "k",
    ))
    .with_models(mock_http::seed_models("openai"));

    let messages = vec![AiMessage::user("hi")];
    let response = provider
        .generate(GenerationParams::new(&messages, "gpt-4.1", 32))
        .await
        .expect("the Responses wire is spoken on /responses");
    assert!(response.content.contains("answered"));
}
