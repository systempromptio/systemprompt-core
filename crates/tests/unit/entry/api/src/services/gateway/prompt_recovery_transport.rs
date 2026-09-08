use std::collections::HashMap;

use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_api::services::gateway::protocol::outbound::gemini::GeminiOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_api::services::gateway::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use systemprompt_api::services::gateway::protocol::outbound::{OutboundAdapter, OutboundCtx};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use systemprompt_security::authz::types::Decision;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::prompt_recovery::{KEY, POLICY, engine, govern, request};

#[tokio::test]
async fn recovery_sends_only_sanitized_bytes_for_every_adapter_and_transport_mode() {
    let cases: Vec<(&str, Box<dyn OutboundAdapter>, Value)> = vec![
        (
            "anthropic",
            Box::new(AnthropicOutbound),
            json!({
                "id":"msg-1", "type":"message", "role":"assistant", "model":"upstream-1",
                "content":[{"type":"text", "text":"ok"}], "stop_reason":"end_turn",
                "usage":{"input_tokens":1, "output_tokens":1}
            }),
        ),
        (
            "openai",
            Box::new(OpenAiChatOutbound),
            json!({
                "id":"chat-1", "object":"chat.completion", "created":1, "model":"upstream-1",
                "choices":[{"index":0, "message":{"role":"assistant", "content":"ok"}, "finish_reason":"stop"}],
                "usage":{"prompt_tokens":1, "completion_tokens":1}
            }),
        ),
        (
            "openai",
            Box::new(OpenAiResponsesOutbound),
            json!({
                "id":"resp-1", "object":"response", "status":"completed", "model":"upstream-1",
                "output":[{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"ok"}]}],
                "usage":{"input_tokens":1, "output_tokens":1}
            }),
        ),
        (
            "gemini",
            Box::new(GeminiOutbound),
            json!({
                "candidates":[{"content":{"role":"model", "parts":[{"text":"ok"}]}, "finishReason":"STOP"}],
                "usageMetadata":{"promptTokenCount":1, "candidatesTokenCount":1}
            }),
        ),
    ];
    for (provider, adapter, response) in cases {
        for streaming in [false, true] {
            for passthrough in [false, true] {
                let server = MockServer::start().await;
                let template = if streaming {
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string("data: [DONE]\n\n")
                } else {
                    ResponseTemplate::new(200).set_body_json(response.clone())
                };
                Mock::given(method("POST"))
                    .respond_with(template)
                    .expect(1)
                    .mount(&server)
                    .await;
                let route = GatewayRoute {
                    id: RouteId::new("recovery-route"),
                    model_pattern: "*".to_owned(),
                    provider: ProviderId::new(provider),
                    upstream_model: Some("upstream-1".to_owned()),
                    extra_headers: HashMap::new(),
                    pricing: None,
                    when: None,
                    requires: None,
                };
                let endpoint = server.uri();
                let mut request = request();
                request.stream = streaming;
                let initial = OutboundCtx {
                    route: &route,
                    endpoint: &endpoint,
                    api_key: "unused",
                    api_key_is_bearer: false,
                    request: &request,
                    upstream_model: "upstream-1",
                    model_limits: None,
                    forward_headers: &[],
                    raw_body: None,
                };
                let raw = adapter.build_body(&initial).unwrap().bytes;
                let mut prepared = adapter
                    .build_body(&OutboundCtx {
                        raw_body: passthrough.then_some(&raw),
                        ..initial
                    })
                    .unwrap();
                let result = govern(&engine(POLICY), &mut request, &mut prepared);
                assert!(
                    matches!(result.evaluation.decision, Decision::Warn { .. }),
                    "{provider}: {:?}",
                    result.evaluation
                );
                let outcome = adapter
                    .send(
                        OutboundCtx {
                            route: &route,
                            endpoint: &endpoint,
                            api_key: "unused",
                            api_key_is_bearer: false,
                            request: &request,
                            upstream_model: "upstream-1",
                            model_limits: None,
                            forward_headers: &[],
                            raw_body: None,
                        },
                        &prepared,
                    )
                    .await;
                assert!(outcome.is_ok(), "{provider}: {}", outcome.err().unwrap());
                let received = server.received_requests().await.unwrap();
                assert_eq!(received.len(), 1);
                assert_eq!(received[0].body, prepared.bytes);
                assert!(!String::from_utf8_lossy(&received[0].body).contains(KEY));
                assert!(
                    String::from_utf8_lossy(&received[0].body).contains("REDACTED_BY_GOVERNANCE")
                );
                assert!(!format!("{:?}", request.flatten_parts()).contains(KEY));
            }
        }
    }
}
