//! Tests for the Anthropic Messages inbound renderer (response + SSE events).

use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{CanonicalContent, ImageSource};
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::{
    AnthropicMessagesInbound, content_to_anthropic_block,
};

fn sample_response() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "claude-x".into(),
        content: vec![
            CanonicalContent::Text("hi".into()),
            CanonicalContent::ToolUse {
                id: "t1".into(),
                name: "ls".into(),
                input: json!({"path": "/"}),
            },
        ],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage {
            input_tokens: 10,
            output_tokens: 5,
            ..CanonicalUsage::default()
        },
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
    }
}

#[test]
fn render_response_serializes_into_anthropic_shape() {
    let inbound = AnthropicMessagesInbound;
    let bytes = inbound.render_response(&sample_response());
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v["type"], "message");
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["model"], "claude-x");
    assert_eq!(v["content"][0]["type"], "text");
    assert_eq!(v["content"][1]["type"], "tool_use");
    assert_eq!(v["stop_reason"], "end_turn");
}

#[test]
fn content_block_helpers_cover_all_variants() {
    let cases = vec![
        CanonicalContent::Text("hello".into()),
        CanonicalContent::Thinking {
            text: "thought".into(),
            signature: Some("sig".into()),
        },
        CanonicalContent::Thinking {
            text: "thought2".into(),
            signature: None,
        },
        CanonicalContent::ToolUse {
            id: "x".into(),
            name: "y".into(),
            input: json!({}),
        },
        CanonicalContent::ToolResult {
            tool_use_id: "tu".into(),
            content: vec![CanonicalContent::Text("ok".into())],
            is_error: false,
        },
        CanonicalContent::Image(ImageSource::Base64 {
            media_type: "image/png".into(),
            data: "AA".into(),
            detail: None,
        }),
        CanonicalContent::Image(ImageSource::Url {
            url: "https://x".into(),
            detail: None,
        }),
    ];
    for c in &cases {
        let v = content_to_anthropic_block(c);
        assert!(v.get("type").and_then(Value::as_str).is_some(), "{v:?}");
    }
}

fn parse_sse_data(bytes: &bytes::Bytes) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|j| serde_json::from_str(j).unwrap_or(Value::Null))
        .collect()
}

#[test]
fn render_event_message_start_uses_fallback_model_when_empty() {
    let inbound = AnthropicMessagesInbound;
    let ev = CanonicalEvent::MessageStart {
        id: "m1".into(),
        model: String::new(),
        usage: CanonicalUsage::default(),
    };
    let frame = inbound.render_event(&ev, "fallback-model").expect("frame");
    let parsed = parse_sse_data(&frame);
    assert!(!parsed.is_empty());
    assert_eq!(parsed[0]["message"]["model"], "fallback-model");
}

#[test]
fn render_event_message_start_uses_event_model_when_set() {
    let inbound = AnthropicMessagesInbound;
    let ev = CanonicalEvent::MessageStart {
        id: "m1".into(),
        model: "claude-event".into(),
        usage: CanonicalUsage::default(),
    };
    let frame = inbound.render_event(&ev, "fallback").expect("frame");
    let parsed = parse_sse_data(&frame);
    assert_eq!(parsed[0]["message"]["model"], "claude-event");
}

#[test]
fn render_event_covers_all_variants() {
    let inbound = AnthropicMessagesInbound;
    let events = vec![
        CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        },
        CanonicalEvent::ContentBlockStart {
            index: 1,
            block: ContentBlockKind::Thinking {
                signature: Some("sig".into()),
            },
        },
        CanonicalEvent::ContentBlockStart {
            index: 2,
            block: ContentBlockKind::ToolUse {
                id: "t".into(),
                name: "do".into(),
            },
        },
        CanonicalEvent::TextDelta {
            index: 0,
            text: "abc".into(),
        },
        CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "...".into(),
        },
        CanonicalEvent::ToolUseDelta {
            index: 2,
            partial_json: "{\"x\":1}".into(),
        },
        CanonicalEvent::ContentBlockStop { index: 0 },
        CanonicalEvent::UsageDelta(CanonicalUsage {
            input_tokens: 1,
            output_tokens: 2,
            ..CanonicalUsage::default()
        }),
        CanonicalEvent::MessageStop {
            id: "m1".into(),
            stop_reason: Some(CanonicalStopReason::ToolUse),
        },
        CanonicalEvent::MessageStop {
            id: "m1".into(),
            stop_reason: None,
        },
        CanonicalEvent::Error("oops \"quoted\\backslash".into()),
    ];
    for ev in events {
        let frame = inbound.render_event(&ev, "m").expect("frame");
        assert!(!frame.is_empty());
    }
}

#[test]
fn render_error_escapes_quotes_and_backslashes() {
    let inbound = AnthropicMessagesInbound;
    let body = inbound.render_error(http::StatusCode::BAD_REQUEST, "boom \"x\\y\"");
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("\\\""));
    assert!(s.contains("\\\\"));
    assert!(s.contains("api_error"));
}

#[test]
fn stop_reason_round_trip_anthropic() {
    let cases = [
        ("end_turn", CanonicalStopReason::EndTurn),
        ("max_tokens", CanonicalStopReason::MaxTokens),
        ("stop_sequence", CanonicalStopReason::StopSequence),
        ("tool_use", CanonicalStopReason::ToolUse),
        ("weird", CanonicalStopReason::Other),
    ];
    for (raw, expected) in cases {
        let parsed = CanonicalStopReason::from_anthropic(raw);
        assert_eq!(parsed, expected);
        let _ = parsed.anthropic_str();
        let _ = parsed.openai_str();
    }
}

#[test]
fn stop_reason_round_trip_openai() {
    let cases = [
        ("stop", CanonicalStopReason::EndTurn),
        ("length", CanonicalStopReason::MaxTokens),
        ("tool_calls", CanonicalStopReason::ToolUse),
        ("function_call", CanonicalStopReason::ToolUse),
        ("weird", CanonicalStopReason::Other),
    ];
    for (raw, expected) in cases {
        let parsed = CanonicalStopReason::from_openai(raw);
        assert_eq!(parsed, expected);
    }
}
