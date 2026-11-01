//! One encoding path for recalled memory: a `<team_memory>` block pushed
//! onto the canonical system prompt lands after the client's own prompt on
//! every wire, and Anthropic carries it as its own uncached system block.

use bytes::Bytes;
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};
use systemprompt_extension::brain::{InjectedFact, RecallBlock};
use systemprompt_gateway::protocol::canonical::{CanonicalRequest, SystemBlock};
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_gateway::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use systemprompt_gateway::protocol::outbound::{OutboundAdapter, OutboundCtx};
use systemprompt_identifiers::BrainFactId;
use systemprompt_wire::WireProtocol;

use super::support;

const CLIENT_PROMPT: &str = "client system prompt";

fn rendered_block() -> String {
    RecallBlock::new("org", Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap())
        .with_fact(InjectedFact {
            id: BrainFactId::try_new("m_00000000-0000-4000-8000-000000000001").expect("fact id"),
            revision: 2,
            source: "doc".to_owned(),
            text: "deploys go through the release PR".to_owned(),
        })
        .render()
}

fn parse(inbound: &dyn InboundAdapter, body: &Value) -> CanonicalRequest {
    let bytes = Bytes::from(serde_json::to_vec(body).expect("serialize"));
    let mut request = inbound.parse_request(&bytes).expect("request parses");
    request.system.push(SystemBlock::text(rendered_block()));
    request
}

fn encode(adapter: &dyn OutboundAdapter, wire: WireProtocol, request: &CanonicalRequest) -> Value {
    let route = systemprompt_catalog_fixtures::hop(
        "upstream",
        wire,
        "https://upstream.invalid/v1",
        "m",
        "m",
    );
    let ctx = OutboundCtx {
        target: &route,
        upstream: &support::api_key_call("http://unused.invalid", "k"),
        http: &reqwest::Client::new(),
        request,
        upstream_model: "m",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: None,
    };
    let prepared = adapter.build_body(&ctx).expect("build");
    serde_json::from_slice(&prepared.bytes).expect("prepared JSON")
}

fn assert_after_client_prompt(text: &str) {
    let client = text.find(CLIENT_PROMPT).expect("client prompt kept");
    let block = text.find(&rendered_block()).expect("memory block present");
    assert!(client < block, "memory must follow the client prompt");
}

#[test]
fn anthropic_carries_the_block_as_a_trailing_uncached_system_block() {
    let request = parse(
        &AnthropicMessagesInbound,
        &json!({
            "model": "claude-x",
            "max_tokens": 64,
            "system": [{ "type": "text", "text": CLIENT_PROMPT }],
            "messages": [{ "role": "user", "content": "hi" }]
        }),
    );

    let body = encode(&AnthropicOutbound, WireProtocol::Anthropic, &request);

    let system = body["system"].as_array().expect("array-form system");
    assert_eq!(system[0]["text"], CLIENT_PROMPT);
    let last = system.last().expect("memory block");
    assert_eq!(last["text"], rendered_block());
    assert!(last.get("cache_control").is_none());
}

#[test]
fn openai_chat_appends_the_block_to_the_system_message() {
    let request = parse(
        &OpenAiChatInbound,
        &json!({
            "model": "gpt-x",
            "messages": [
                { "role": "system", "content": CLIENT_PROMPT },
                { "role": "user", "content": "hi" }
            ]
        }),
    );

    let body = encode(&OpenAiChatOutbound, WireProtocol::OpenAiChat, &request);

    let system = body["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .find(|message| message["role"] == "system")
        .expect("system message");
    assert_after_client_prompt(system["content"].as_str().expect("string content"));
}

#[test]
fn openai_responses_appends_the_block_to_the_instructions() {
    let request = parse(
        &OpenAiResponsesInbound,
        &json!({
            "model": "gpt-x",
            "instructions": CLIENT_PROMPT,
            "input": [{ "type": "message", "role": "user", "content": "hi" }]
        }),
    );

    let body = encode(
        &OpenAiResponsesOutbound,
        WireProtocol::OpenAiResponses,
        &request,
    );

    assert_after_client_prompt(body["instructions"].as_str().expect("instructions"));
}
