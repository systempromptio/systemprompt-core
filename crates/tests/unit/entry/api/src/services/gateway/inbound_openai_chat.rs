//! Tests for the OpenAI Chat Completions inbound adapter — parsing, response
//! rendering, streaming chunks, terminal frames, and error envelopes.

use bytes::Bytes;
use http::StatusCode;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalToolChoice, ReasoningEffort, ResponseFormat, Role,
};
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::{InboundAdapter, InboundParseError};
use systemprompt_models::services::WireProtocol;

fn parse(body: &str) -> systemprompt_api::services::gateway::protocol::canonical::CanonicalRequest {
    OpenAiChatInbound
        .parse_request(&Bytes::from(body.to_owned()))
        .expect("parse")
}

#[test]
fn wire_name_and_passthrough() {
    assert_eq!(OpenAiChatInbound.wire_name(), "openai.chat");
    assert_eq!(
        OpenAiChatInbound.passthrough_wire(),
        Some(WireProtocol::OpenAiChat)
    );
}

#[test]
fn parse_invalid_json_and_missing_model() {
    let err = OpenAiChatInbound
        .parse_request(&Bytes::from_static(b"nope"))
        .expect_err("should fail");
    assert!(matches!(err, InboundParseError::InvalidJson(_)));

    let err = OpenAiChatInbound
        .parse_request(&Bytes::from_static(br#"{"messages":[]}"#))
        .expect_err("should fail");
    assert!(matches!(err, InboundParseError::MissingField("model")));
}

#[test]
fn parse_system_and_developer_fold_into_system() {
    let req = parse(
        r#"{"model":"gpt-4o","messages":[
            {"role":"system","content":"one"},
            {"role":"developer","content":"two"},
            {"role":"user","content":"hi"}
        ]}"#,
    );
    assert_eq!(req.system.as_deref(), Some("one\ntwo"));
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, Role::User);
}

#[test]
fn parse_max_completion_tokens_wins_over_max_tokens() {
    let req = parse(r#"{"model":"m","max_tokens":10,"max_completion_tokens":20,"messages":[]}"#);
    assert_eq!(req.max_tokens, 20);
    let req = parse(r#"{"model":"m","max_tokens":10,"messages":[]}"#);
    assert_eq!(req.max_tokens, 10);
    let req = parse(r#"{"model":"m","messages":[]}"#);
    assert_eq!(req.max_tokens, 4096);
}

#[test]
fn parse_assistant_tool_calls_and_tool_result_round_trip() {
    let req = parse(
        r#"{"model":"m","messages":[
            {"role":"user","content":"weather?"},
            {"role":"assistant","content":null,"tool_calls":[
                {"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"Berlin\"}"}}
            ]},
            {"role":"tool","tool_call_id":"call_1","content":"sunny"}
        ]}"#,
    );
    assert_eq!(req.messages.len(), 3);
    match &req.messages[1].content[0] {
        CanonicalContent::ToolUse {
            id, name, input, ..
        } => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "get_weather");
            assert_eq!(input["city"], "Berlin");
        },
        other => panic!("expected ToolUse, got {other:?}"),
    }
    assert_eq!(req.messages[2].role, Role::Tool);
    match &req.messages[2].content[0] {
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            ..
        } => {
            assert_eq!(tool_use_id, "call_1");
            assert!(matches!(&content[0], CanonicalContent::Text(t) if t == "sunny"));
        },
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[test]
fn parse_tool_message_without_call_id_fails() {
    let err = OpenAiChatInbound
        .parse_request(&Bytes::from_static(
            br#"{"model":"m","messages":[{"role":"tool","content":"x"}]}"#,
        ))
        .expect_err("should fail");
    assert!(matches!(
        err,
        InboundParseError::MissingField("tool_call_id")
    ));
}

#[test]
fn parse_unknown_role_is_unsupported() {
    let err = OpenAiChatInbound
        .parse_request(&Bytes::from_static(
            br#"{"model":"m","messages":[{"role":"robot","content":"x"}]}"#,
        ))
        .expect_err("should fail");
    assert!(matches!(
        err,
        InboundParseError::Unsupported {
            field: "messages.role",
            ..
        }
    ));
}

#[test]
fn parse_tools_tool_choice_effort_and_response_format() {
    let req = parse(
        r#"{"model":"m","messages":[],
            "tools":[{"type":"function","function":{"name":"f","description":"d","parameters":{"type":"object"}}}],
            "tool_choice":{"type":"function","function":{"name":"f"}},
            "reasoning_effort":"minimal",
            "response_format":{"type":"json_schema","json_schema":{"name":"out","strict":true,"schema":{"type":"object"}}},
            "stream":true}"#,
    );
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.tools[0].name, "f");
    assert!(matches!(
        req.tool_choice,
        Some(CanonicalToolChoice::Tool(ref n)) if n == "f"
    ));
    assert_eq!(req.reasoning_effort, Some(ReasoningEffort::Low));
    assert!(matches!(
        req.response_format,
        Some(ResponseFormat::JsonSchema { ref name, strict: true, .. }) if name == "out"
    ));
    assert!(req.stream);
}

#[test]
fn parse_data_uri_image_becomes_base64_source() {
    let req = parse(
        r#"{"model":"m","messages":[{"role":"user","content":[
            {"type":"text","text":"look"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,AAAA","detail":"low"}}
        ]}]}"#,
    );
    match &req.messages[0].content[1] {
        CanonicalContent::Image(
            systemprompt_api::services::gateway::protocol::canonical::ImageSource::Base64 {
                media_type,
                data,
                ..
            },
        ) => {
            assert_eq!(media_type, "image/png");
            assert_eq!(data, "AAAA");
        },
        other => panic!("expected base64 image, got {other:?}"),
    }
}

fn sample_response() -> CanonicalResponse {
    CanonicalResponse {
        id: "chatcmpl_1".into(),
        model: "gpt-x".into(),
        content: vec![
            CanonicalContent::Text("answer".into()),
            CanonicalContent::ToolUse {
                id: "t1".into(),
                name: "fn".into(),
                input: json!({"a": 1}),
                signature: None,
            },
        ],
        stop_reason: Some(CanonicalStopReason::ToolUse),
        usage: CanonicalUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 3,
            cache_creation_tokens: 0,
            total_tokens: 18,
        },
        ..Default::default()
    }
}

#[test]
fn render_response_is_chat_completion_shape() {
    let bytes = OpenAiChatInbound.render_response(&sample_response());
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["id"], "chatcmpl_1");
    assert_eq!(v["model"], "gpt-x");
    let choice = &v["choices"][0];
    assert_eq!(choice["index"], 0);
    assert_eq!(choice["finish_reason"], "tool_calls");
    assert_eq!(choice["message"]["role"], "assistant");
    assert_eq!(choice["message"]["content"], "answer");
    let call = &choice["message"]["tool_calls"][0];
    assert_eq!(call["id"], "t1");
    assert_eq!(call["function"]["name"], "fn");
    assert_eq!(call["function"]["arguments"], "{\"a\":1}");
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
    assert_eq!(v["usage"]["total_tokens"], 15);
    assert_eq!(v["usage"]["prompt_tokens_details"]["cached_tokens"], 3);
}

fn chunk_json(bytes: &Bytes) -> Value {
    let s = std::str::from_utf8(bytes).expect("utf8");
    let data = s
        .lines()
        .find_map(|l| l.strip_prefix("data: "))
        .expect("data line");
    serde_json::from_str(data).expect("chunk json")
}

#[test]
fn render_stream_start_text_and_tool_deltas() {
    let start = OpenAiChatInbound
        .render_event(
            &CanonicalEvent::MessageStart {
                id: "x".into(),
                model: "gpt-x".into(),
                usage: CanonicalUsage::default(),
            },
            "gpt-x",
        )
        .expect("frame");
    let v = chunk_json(&start);
    assert_eq!(v["object"], "chat.completion.chunk");
    assert_eq!(v["choices"][0]["delta"]["role"], "assistant");

    let text = OpenAiChatInbound
        .render_event(
            &CanonicalEvent::TextDelta {
                index: 0,
                text: "hi".into(),
            },
            "gpt-x",
        )
        .expect("frame");
    assert_eq!(chunk_json(&text)["choices"][0]["delta"]["content"], "hi");

    let tool_start = OpenAiChatInbound
        .render_event(
            &CanonicalEvent::ContentBlockStart {
                index: 1,
                block: ContentBlockKind::ToolUse {
                    id: "t1".into(),
                    name: "fn".into(),
                    signature: None,
                },
            },
            "gpt-x",
        )
        .expect("frame");
    let tc = &chunk_json(&tool_start)["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tc["index"], 1);
    assert_eq!(tc["id"], "t1");
    assert_eq!(tc["function"]["name"], "fn");

    let args = OpenAiChatInbound
        .render_event(
            &CanonicalEvent::ToolUseDelta {
                index: 1,
                partial_json: "{\"a\"".into(),
            },
            "gpt-x",
        )
        .expect("frame");
    assert_eq!(
        chunk_json(&args)["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        "{\"a\""
    );

    assert!(
        OpenAiChatInbound
            .render_event(&CanonicalEvent::ContentBlockStop { index: 0 }, "gpt-x")
            .is_none()
    );
}

#[test]
fn render_terminal_emits_final_chunk_usage_and_done() {
    let snapshot = sample_response();
    let bytes = OpenAiChatInbound
        .render_terminal_event(
            &CanonicalEvent::MessageStop {
                id: "chatcmpl_1".into(),
                stop_reason: Some(CanonicalStopReason::EndTurn),
            },
            &snapshot,
            "gpt-x",
        )
        .expect("terminal frame");
    let s = std::str::from_utf8(&bytes).expect("utf8");
    assert!(s.ends_with("data: [DONE]\n\n"));
    let v = chunk_json(&bytes);
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
}

#[test]
fn render_error_is_openai_envelope() {
    let bytes = OpenAiChatInbound.render_error(StatusCode::BAD_REQUEST, "bad \"input\"");
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v["error"]["type"], "api_error");
    assert_eq!(v["error"]["message"], "bad \"input\"");
}
